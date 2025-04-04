use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::Duration};

use moq_karp::Dimensions;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_codecs::{Timestamp, VideoFrame};
use web_time::Instant;

use super::{ControlsRecv, RendererStatus, StatusSend};

struct AudioPlayer {
	status: StatusSend,

	state: RendererStatus,
	scheduled: bool,
	resolution: Dimensions,

	// Used to determine which frame to render next.
	latency: Duration,
	latency_ref: Option<(Instant, Timestamp)>,
}

impl AudioPlayer {
}
