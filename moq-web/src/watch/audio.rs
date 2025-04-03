use moq_async::FuturesExt;

use crate::Result;

pub struct Audio {
    pub track: moq_karp::TrackConsumer,

    // Audio information
    codec: String,
    sample_rate: u32,
    channels: u32,

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

        Ok(Self {
            track,
            codec: info.codec.to_string(),
            sample_rate: info.sample_rate as u32,
            channels: info.channel_count as u32,
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
            }
        }
    }
}