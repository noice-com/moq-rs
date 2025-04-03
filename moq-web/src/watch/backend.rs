use moq_karp::BroadcastConsumer;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::prelude::*;

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

#[wasm_bindgen]
impl Backend {
	// Add a function to register an audio callback from JavaScript
	pub fn register_audio_callback(&self, _callback: js_sys::Function) -> Result<()> {
		tracing::info!("Audio callback registered from JavaScript");
		// The callback will be called from bridge.ts
		// This function just serves as a marker to confirm the backend supports audio callbacks
		Ok(())
	}
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

						// Create a simpler approach to error handling
						let track_result = self.broadcast.as_mut().unwrap().track(&info.track);
						match track_result {
							Ok(mut track) => {
								track.set_latency(self.controls.latency.get());

								// Create a new audio handler
								match Audio::new(track, info.clone()) {
									Ok(mut audio) => {
										// Set volume
										audio.set_volume(self.controls.volume.get() as f32);

										tracing::info!("Audio track initialized successfully, starting processing");

										// Process audio frames in a separate task
										spawn_local(async move {
											if let Err(err) = audio.process().await {
												tracing::error!(?err, "Audio processing error");
											} else {
												tracing::info!("Audio processing completed normally");
											}
										});
									},
									Err(err) => {
										tracing::warn!("Failed to initialize audio handler: {:?}", err);
									}
								}
							},
							Err(err) => {
								tracing::warn!("Failed to get audio track: {:?}", err);
							}
						}
					} else {
						tracing::info!("No audio track found");
					}
				},
				Some(frame) = async { self.video.as_mut()?.frame().await.transpose() } => {
					self.renderer.push(frame?);
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
				volume = self.controls.volume.next() => {
					let volume = volume.ok_or(Error::Closed)?;
					if let Some(audio) = &self.audio {
						// Need to cast to mutable reference to update volume
						let audio_ptr = audio as *const Audio as *mut Audio;
						unsafe {
							(*audio_ptr).set_volume(volume as f32);
						}
					}
				},
				else => return Ok(()),
			}
		}
	}
}