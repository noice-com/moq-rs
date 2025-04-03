// This script initializes the Opus player in the browser environment
// It imports the actual player implementation and adds the functions to window
import {
    initOpusPlayer,
    decodeOpusFrame,
    setVolume,
    cleanupOpusPlayer,
    testOpusDecoder
} from './opus_player.js';

// Only execute in browser environment
if (typeof window !== 'undefined') {
    console.log("Initializing Opus player for MoQ...");
    
    // Add the functions to window object for Rust WebAssembly to access
    window.init_opus_player = initOpusPlayer;
    window.decode_opus_frame = decodeOpusFrame;
    window.set_volume = setVolume;
    window.cleanup_opus_player = cleanupOpusPlayer;
    window.test_opus_decoder = testOpusDecoder;
    
    console.log("Opus player functions registered to window object");
}