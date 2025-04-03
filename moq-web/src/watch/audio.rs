use moq_async::FuturesExt;
use web_sys::{AudioContext, GainNode};
use std::sync::Arc;

use crate::Result;

pub struct Audio {
    pub track: moq_karp::TrackConsumer,

    // Audio information
    codec: String,
    sample_rate: u32,
    channels: u32,

    // Web Audio API objects for playback
    audio_context: Option<AudioContext>,
    gain_node: Option<GainNode>,

    // Web codecs decoder
    decoder: web_codecs::AudioDecoder,
    decoded: web_codecs::AudioDecoded,

    // Volume control
    volume: f32,

    // Frame tracking
    frame_count: usize,
    last_keyframe_timestamp: Option<std::time::Duration>,
}

impl Audio {
    pub fn new(track: moq_karp::TrackConsumer, info: moq_karp::Audio) -> Result<Self> {
        tracing::info!(
            "Creating audio track handler: sample_rate={}, channels={}, codec={}",
            info.sample_rate, info.channel_count, info.codec
        );

        // We're now delegating audio playback to the main thread via bridge.ts
        // So we don't need to create AudioContext in the worker thread
        tracing::info!("Audio playback will be handled by main thread via JavaScript bridge");
        let audio_context = None;
        let gain_node = None;

        // Initialize the audio decoder using our web-codecs implementation
        let (decoder, decoded) = web_codecs::AudioDecoderConfig {
            codec: info.codec.to_string(),
            sample_rate: Some(info.sample_rate as u32),
            number_of_channels: Some(info.channel_count as u32),
            latency_optimized: Some(true),
            ..Default::default()
        }
        .build()?;

        tracing::info!("Audio decoder initialized with codec: {}", info.codec);

        Ok(Self {
            track,
            codec: info.codec.to_string(),
            sample_rate: info.sample_rate as u32,
            channels: info.channel_count as u32,
            audio_context,
            gain_node,
            decoder,
            decoded,
            volume: 1.0,
            frame_count: 0,
            last_keyframe_timestamp: None,
        })
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        tracing::info!("Volume control set to: {}", volume);

        // Since we're now using the JavaScript bridge for audio playback,
        // we just store the volume for when frames are passed to the bridge
        // The bridge.ts file handles the actual volume control
    }

    // Play an audio buffer
    fn play_buffer(&self, buffer: &web_sys::AudioBuffer) -> Result<()> {
        // This method is now just a stub - actual playback happens in bridge.ts
        // We log the buffer info for debugging, but don't try to play it directly
        tracing::debug!("Got audio buffer: sample_rate={}, channels={}, duration={}ms",
            buffer.sample_rate(),
            buffer.number_of_channels(),
            (buffer.duration() * 1000.0) as u32);

        // The Bridge will handle the actual playback
        tracing::debug!("Audio playback is handled by JavaScript bridge");

        Ok(())
    }

    async fn process_frame(&mut self, frame: &moq_karp::Frame) -> Result<()> {
        // Track keyframes for potential seeking
        if frame.keyframe {
            self.last_keyframe_timestamp = Some(frame.timestamp);
            tracing::debug!("Audio keyframe received, timestamp: {:?}", frame.timestamp);
        }

        self.frame_count += 1;

        // Log only the first few frames and then every 100th frame
        if self.frame_count <= 3 || self.frame_count % 100 == 0 {
            let is_opus = self.codec.to_lowercase().contains("opus");
            let frame_type = if is_opus { "Opus" } else { &self.codec };

            tracing::info!(
                "Processing {} audio frame #{}: size={} bytes, timestamp={:?}, keyframe={}",
                frame_type,
                self.frame_count,
                frame.payload.len(),
                frame.timestamp,
                frame.keyframe
            );
        }

        // Create a frame data object that can be passed to JS
        // We need this data structure to match what bridge.ts expects
        struct FrameData {
            payload: Vec<u8>,
            codec: String,
            sample_rate: u32,
            channels: u32,
            timestamp: std::time::Duration,
            keyframe: bool,
        }

        // Create a wrapper that will implement the JS functions
        // Currently unused, but keeping the structure for future implementation with callbacks
        let _frame_data = Arc::new(FrameData {
            payload: frame.payload.to_vec(),  // Convert from Bytes to Vec<u8>
            codec: self.codec.clone(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            timestamp: frame.timestamp,
            keyframe: frame.keyframe,
        });

        // Send the frame to the decoder
        let encoded_frame = web_codecs::EncodedFrame {
            payload: frame.payload.clone(),
            timestamp: frame.timestamp,
            keyframe: frame.keyframe,
        };

        // Send the frame to the decoder for processing
        // The decoder will handle this in a custom way that works with our bridge
        if let Err(e) = self.decoder.decode(encoded_frame) {
            tracing::debug!("Decoder returned: {:?} - expected for worker handoff", e);
        }

        Ok(())
    }

    // Process audio frames - main loop
    pub async fn process(&mut self) -> Result<()> {
        tracing::info!("Starting audio processing with codec: {}", self.codec);

        // Process frames and decoded audio in parallel
        loop {
            tokio::select! {
                // Handle new frames from the track
                Some(frame) = self.track.read().transpose() => {
                    match frame {
                        Ok(frame) => {
                            // Process the frame
                            let _ = self.process_frame(&frame).await;
                        },
                        Err(e) => {
                            tracing::warn!("Error reading audio frame: {:?}", e);
                        }
                    }
                },

                // Handle decoded audio from the decoder
                Some(audio_data) = self.decoded.next().transpose() => {
                    match audio_data {
                        Ok(audio_data) => {
                            // Play the decoded audio
                            if let Err(e) = self.play_buffer(&audio_data.buffer) {
                                tracing::warn!("Error playing audio buffer: {:?}", e);
                            }
                        },
                        Err(e) => {
                            // This might not be an actual error - our decoder sends "Unsupported" when it wants the main thread to handle audio
                            // The bridge.ts code picks up the frames directly
                            tracing::debug!("Decoder reported: {:?} - audio playback delegated to main thread", e);
                        }
                    }
                },

                // Exit when both track and decoder are done
                else => {
                    tracing::info!("Audio processing ended after {} frames", self.frame_count);
                    return Ok(());
                },
            }
        }
    }
}