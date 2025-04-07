use wasm_bindgen::JsValue;
use web_codecs::AudioFrame;


pub struct AudioPlayer {
	// status: StatusSend,
	// audiocontext: web_sys::AudioContext,
	// audioworklet: web_sys::AudioWorkletNode,
	// workletnode: web_sys::AudioWorkletNode,
}

impl AudioPlayer {
	pub fn new() -> Result<AudioPlayer, JsValue> {
		// Create the AudioContext
		let context = web_sys::AudioContext::new()?;
		// let options = web_sys::AudioWorkletNodeOptions::new();
		// let worklet = context.audio_worklet()?;
		// let _ = worklet.add_module("worklet.js")?;
		// let worklet_node = web_sys::AudioWorkletNode::new_with_options(&context, "WasmProcessor", &options)?;
		// worklet_node.connect_with_audio_node(&context.destination())?;
		Ok(AudioPlayer {
			// status,
			// audiocontext: context,
			// audioworklet: worklet,
			// workletnode,
		})
	}
	pub fn play(&mut self, frame: AudioFrame) {

	}
}
