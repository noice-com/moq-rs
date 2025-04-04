use moq_async::FuturesExt;

use crate::Result;

pub struct Audio {
	pub track: moq_karp::TrackConsumer,

	decoder: web_codecs::AudioDecoder,
	decoded: web_codecs::AudioDecoded,
}

impl Audio {
	pub fn new(track: moq_karp::TrackConsumer, info: moq_karp::Audio) -> Result<Self> {
		// Construct the Audio decoder
		let (decoder, decoded) = web_codecs::AudioDecoderConfig {
			codec: info.codec.to_string(),

			..Default::default()
		}
		.build()?;

		Ok(Self {
			track,
			decoder,
			decoded,
		})
	}

	pub async fn frame(&mut self) -> Result<Option<web_codecs::AudioFrame>> {
		loop {
			tokio::select! {
				Some(frame) = self.track.read().transpose() => {
					let frame = frame?;

					let frame = web_codecs::EncodedFrame {
						payload: frame.payload,
						timestamp: frame.timestamp,
						keyframe: frame.keyframe,
					};

					self.decoder.decode(frame)?;
				},
				Some(frame) = self.decoded.next().transpose() => return Ok(Some(frame?)),
				else => return Ok(None),
			}
		}
	}
}
