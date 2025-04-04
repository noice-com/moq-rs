use web_codecs::AudioFrame;

use web_sys::{AudioContext, AudioWorkletNode, AudioWorkletNodeOptions};
use super::StatusSend;

pub struct AudioPlayer {
	status: StatusSend,
	// audiocontext: web_sys::AudioContext,
	// audioworklet: web_sys::AudioWorkletNode,
	// workletnode: web_sys::AudioWorkletNode,
}

impl AudioPlayer {
	pub fn new(status: StatusSend) -> Self {
		// Create the AudioContext
		let context = web_sys::AudioContext::new().unwrap();
		let options = web_sys::AudioWorkletNodeOptions::new();
		Self {
			status,
		}
	}
	pub fn play(&mut self, frame: AudioFrame) {

	}
}
