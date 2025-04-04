use moq_karp::BroadcastConsumer;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::prelude::*;
use super::wasm_audio::wasm_audio;

use super::{Audio, ControlsRecv, Renderer, StatusSend, Video};
use crate::{Connect, ConnectionStatus, Error, Result};

#[wasm_bindgen]
pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	connect: Option<Connect>,
	broadcast: Option<BroadcastConsumer>,
	video: Option<Video>,
	audio: Option<Audio>,

	renderer: Renderer,
}

impl Backend {
	pub fn new(controls: ControlsRecv, status: StatusSend) -> Self {
		Self {
			renderer: Renderer::new(controls.clone(), status.clone()),

			controls,
			status,

			connect: None,
			broadcast: None,
			video: None,
			audio: None,
		}
	}

	pub fn start(mut self) {
		spawn_local(async move {
			if let Err(err) = self.run().await {
				tracing::error!(?err, "backend error");
				self.status.error.set(Some(err));
			}
		});
	}

	async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				url = self.controls.url.next() => {
					let url = url.ok_or(Error::Closed)?;

					self.broadcast = None;
					self.video = None;
					self.audio = None;

					if let Some(url) = url {
						self.connect = Some(Connect::new(url));
						self.status.connection.update(ConnectionStatus::Connecting);
					} else {
						self.connect = None;
						self.status.connection.update(ConnectionStatus::Disconnected);
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let path = self.connect.take().unwrap().path;

					tracing::info!(?path, "Connected, loading broadcast");
					let broadcast = moq_karp::BroadcastConsumer::new(session?, path);
					self.status.connection.update(ConnectionStatus::Connected);

					self.broadcast = Some(broadcast);
					self.connect = None;
				},
				Some(catalog) = async { Some(self.broadcast.as_mut()?.next_catalog().await) } => {
					let catalog = match catalog? {
						Some(catalog) => {
							self.status.connection.update(ConnectionStatus::Live);
							catalog.clone()
						},
						None => {
							// There's no catalog, so the stream is offline.
							// Note: We keep trying because the stream might come online later.
							self.status.connection.update(ConnectionStatus::Offline);
							self.video = None;
							self.audio = None;
							continue;
						},
					};

					// Handle video track
					// TODO add an ABR module
					if let Some(info) = catalog.video.first() {
						tracing::info!(?info, "Loading video track");

						let mut track = self.broadcast.as_mut().unwrap().track(&info.track)?;
						track.set_latency(self.controls.latency.get());
						self.renderer.set_resolution(info.resolution);

						let video = Video::new(track, info.clone())?;
						self.video = Some(video);
					} else {
						tracing::info!("No video track found");

						self.renderer.set_resolution(Default::default());
						self.video = None;
					}

					// Handle audio track
					if let Some(info) = catalog.audio.first() {
						tracing::info!(?info, "Loading audio track");
						let mut track = self.broadcast.as_mut().unwrap().track(&info.track)?;

						let audio = Audio::new(track, info.clone())?;
						self.audio = Some(audio);

					} else {
						tracing::info!("No audio track found");
					}
				},
				Some(frame) = async { self.video.as_mut()?.frame().await.transpose() } => {
					self.renderer.push(frame?);
				},
				Some(frame) = async { self.audio.as_mut()?.frame().await.transpose() } => {
					if let Ok(frame) = frame {
						// Create a processor function that will be called by the audio worklet
						let processor = Box::new(move |buf: &mut [f32]| {
							// Fill the buffer with audio data from the frame
							// This is a simplified example - in a real implementation,
							// you would need to properly decode the audio data
							for i in 0..buf.len() {
								// For now, just generate a simple sine wave as a placeholder
								// In a real implementation, you would decode the actual audio data
								buf[i] = 0.0;
							}
							true // Return true to continue processing
						});

						// Initialize the audio context and processor
						match wasm_audio(processor).await {
							Ok(_ctx) => {
								// Audio context initialized successfully
								tracing::debug!("Audio context initialized");
							},
							Err(e) => {
								tracing::error!("Failed to initialize audio context: {:?}", e);
							}
						};
					}
				},
				_ = self.controls.paused.next() => {
					// TODO temporarily unsubscribe on pause
				},
				latency = self.controls.latency.next() => {
					let latency = latency.ok_or(Error::Closed)?;
					if let Some(video) = self.video.as_mut() {
						 video.track.set_latency(latency);
					}
				},
				else => return Ok(()),
			}
		}
	}
}
