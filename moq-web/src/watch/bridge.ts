import * as Comlink from "comlink";

import * as Rust from "@rust";
export type { Watch } from "@rust";

// Define interfaces for custom events
interface VolumeChangeDetail {
  volume: number;
}

// Handle audio processing in the main thread
interface AudioDecoderHandler {
  audioContext?: AudioContext;
  gainNode?: GainNode;
  volume: number;
  // Store raw frames for debugging
  lastFrames: Uint8Array[];
  // WebCodecs AudioDecoder
  webCodecsDecoder?: AudioDecoder;
  // Track ongoing decodes to avoid overlapping
  pendingDecodes: number;
  // Buffer for decoded audio samples
  audioSamples: Float32Array[];
}

export class Bridge {
  private audioHandler: AudioDecoderHandler | null = null;

  async watch(): Promise<Rust.Watch & Comlink.ProxyMarked> {
    // Proxy the Watch instance
    const watch = Comlink.proxy(new Rust.Watch());

    // Wrap the status() method to proxy its result
    const originalStatus = watch.status.bind(watch);
    const anyWatch = watch as any;

    // Add a direct getter for the backend
    let watchBackend: any = null;
    Object.defineProperty(anyWatch, 'getBackend', {
      value: async function() {
        if (watchBackend) return watchBackend;

        try {
          const status = await originalStatus();
          watchBackend = status.backend;
          return watchBackend;
        } catch (e) {
          console.error("Error getting backend:", e);
          return null;
        }
      }
    });

    // Check for audio decoder every 500ms until found
    let audioCheckInterval = setInterval(async () => {
      if (this.audioHandler) {
        clearInterval(audioCheckInterval);
        return;
      }

      try {
        const backend = await anyWatch.getBackend();
        if (backend) {
          console.log("FOUND BACKEND! Setting up audio handling in main thread");
          this.setupAudioHandler(backend);
          clearInterval(audioCheckInterval);
        }
      } catch (e) {
        console.log("Still waiting for backend to be available...");
      }
    }, 500);

    anyWatch.status = Comlink.proxy(async () => {
      const statusObj = await originalStatus();
      const statusProxy = Comlink.proxy(statusObj);
      return statusProxy;
    });

    return watch;
  }

  private setupAudioHandler(backend: any): void {
    try {
      console.log("Setting up audio handler with backend:", backend);

      if (!backend) {
        console.error("Backend is not defined!");
        return;
      }

      if (!backend.register_audio_callback) {
        console.error("register_audio_callback method is not available on backend!");
        console.log("Available properties:", Object.getOwnPropertyNames(backend));
        return;
      }

      // Modern browsers require user interaction before creating AudioContext
      const setupAudio = () => {
        try {
          // Create audio context and connect directly to destination
          const audioContext = new AudioContext();
          console.log("AudioContext created:", audioContext.state);

          // Resume the audio context if it's suspended
          if (audioContext.state === 'suspended') {
            audioContext.resume().then(() => {
              console.log("AudioContext resumed");
            }).catch(err => {
              console.error("Failed to resume AudioContext:", err);
            });
          }

          // Create a gain node fixed at volume 1.0
          const gainNode = audioContext.createGain();
          gainNode.gain.value = 1.0; // Fixed volume at maximum
          gainNode.connect(audioContext.destination);

          // Initialize our audio handler with storage for frames
          this.audioHandler = {
            audioContext,
            gainNode,
            volume: 1.0,
            lastFrames: [],
            pendingDecodes: 0,
            audioSamples: []
          };

          // Check if WebCodecs is supported
          if (typeof AudioDecoder !== 'undefined') {
            console.log("WebCodecs AudioDecoder is supported!");
            try {
              this.initWebCodecsDecoder();
            } catch (e) {
              console.error("Error initializing WebCodecs decoder:", e);
            }
          } else {
            console.warn("WebCodecs AudioDecoder is not supported in this browser");
          }

          // Set up a frame callback
          const frameCallback = Comlink.proxy((frameData: any) => {
            console.log("✅ FRAME CALLBACK RECEIVED A FRAME!");
            return this.handleAudioFrame(frameData);
          });

          // Register the callback to receive audio frames from Rust
          console.log("📞 Registering audio callback on backend...");
          try {
            backend.register_audio_callback(frameCallback);
            console.log("✅ Successfully registered audio callback!");
          } catch (e) {
            console.error("❌ ERROR registering audio callback:", e);
          }

          console.log("Audio handler set up successfully");

          // Create a test tone to verify audio is working
          this.playTestTone();

          // Add a debug button to the page
          this.addDebugButton();
        } catch (e) {
          console.error("Error in setupAudio:", e);
        }
      };

      // Set up the audio on first click/touch to satisfy browser autoplay policies
      const setupOnUserInteraction = () => {
        setupAudio();
        document.removeEventListener('click', setupOnUserInteraction);
        document.removeEventListener('touchstart', setupOnUserInteraction);
      };

      // Try to set up immediately, but also listen for user interaction
      setupAudio();
      document.addEventListener('click', setupOnUserInteraction);
      document.addEventListener('touchstart', setupOnUserInteraction);
    } catch (e) {
      console.error("Error setting up audio handler:", e);
    }
  }

  // Play a test tone to verify audio is working
  private playTestTone(): void {
    if (!this.audioHandler?.audioContext) return;

    try {
      const oscillator = this.audioHandler.audioContext.createOscillator();
      oscillator.type = 'sine';
      oscillator.frequency.setValueAtTime(440, this.audioHandler.audioContext.currentTime); // A4 note

      oscillator.connect(this.audioHandler.gainNode!);
      oscillator.start();
      oscillator.stop(this.audioHandler.audioContext.currentTime + 0.2); // Short beep

      console.log("Test tone played");
    } catch (e) {
      console.error("Error playing test tone:", e);
    }
  }

  // Add a debug button to help diagnose issues
  private addDebugButton(): void {
    const button = document.createElement('button');
    button.textContent = "Debug Audio";
    button.style.position = 'fixed';
    button.style.bottom = '10px';
    button.style.right = '10px';
    button.style.zIndex = '9999';
    button.style.color = 'black';

    button.addEventListener('click', () => {
      this.debugAudio();
    });

    document.body.appendChild(button);
  }

  // Debugging function to dump audio info
  private debugAudio(): void {
    if (!this.audioHandler) {
      console.log("No audio handler available");
      return;
    }

    console.log("Audio Context:", {
      state: this.audioHandler.audioContext?.state,
      sampleRate: this.audioHandler.audioContext?.sampleRate,
      baseLatency: this.audioHandler.audioContext?.baseLatency,
    });

    console.log("WebCodecs Decoder:", {
      available: typeof AudioDecoder !== 'undefined',
      initialized: !!this.audioHandler.webCodecsDecoder,
      state: this.audioHandler.webCodecsDecoder?.state,
      pendingDecodes: this.audioHandler.pendingDecodes
    });

    console.log("Frames received:", this.audioHandler.lastFrames.length);
    console.log("Decoded samples available:", this.audioHandler.audioSamples.length > 0);

    // First try to play any decoded samples we have
    if (this.audioHandler.audioSamples.length > 0 && this.audioHandler.audioContext) {
      this.playWebCodecsAudio(
        this.audioHandler.audioSamples,
        this.audioHandler.audioContext.sampleRate,
        Date.now() * 1000
      );
    }
    // If no decoded samples, try to play raw frame
    else if (this.audioHandler.lastFrames.length > 0 && this.audioHandler.audioContext) {
      this.playRawAudioData(this.audioHandler.lastFrames[this.audioHandler.lastFrames.length - 1]);
    }

    // Try to reset the decoder if it's in an error state
    if (this.audioHandler.webCodecsDecoder &&
        this.audioHandler.webCodecsDecoder.state === "closed") {
      console.log("Reinitializing WebCodecs decoder");
      this.initWebCodecsDecoder();
    }
  }

  // Try to play raw audio data as PCM - this helps test if audio output works at all
  private playRawAudioData(rawData: Uint8Array): void {
    if (!this.audioHandler?.audioContext) return;

    try {
      const audioContext = this.audioHandler.audioContext;
      const sampleRate = 48000; // Assume 48kHz sample rate for Opus
      const numChannels = 2; // Assume stereo

      // Create an audio buffer
      const audioBuffer = audioContext.createBuffer(
        numChannels,
        rawData.length / numChannels,
        sampleRate
      );

      // Convert the raw byte data to float32 (very naive conversion)
      for (let channel = 0; channel < numChannels; channel++) {
        const channelData = audioBuffer.getChannelData(channel);
        for (let i = 0; i < channelData.length; i++) {
          // Convert byte to float in range [-1, 1]
          channelData[i] = (rawData[i * numChannels + channel] - 128) / 128.0;
        }
      }

      // Create a buffer source and play it
      const source = audioContext.createBufferSource();
      source.buffer = audioBuffer;
      source.connect(this.audioHandler.gainNode!);
      source.start();

      console.log("Playing raw audio data as PCM");
    } catch (e) {
      console.error("Error playing raw audio data:", e);
    }
  }

  // Initialize the WebCodecs AudioDecoder
  private initWebCodecsDecoder(): void {
    if (!this.audioHandler) return;

    try {
      // Set up the output handler for decoded audio data
      const outputCallback = (frame: AudioData) => {
        try {
          console.log("Received decoded audio data:", {
            format: frame.format,
            sampleRate: frame.sampleRate,
            numberOfFrames: frame.numberOfFrames,
            numberOfChannels: frame.numberOfChannels,
            timestamp: frame.timestamp
          });

          // Extract audio data from the frame
          const numberOfChannels = frame.numberOfChannels;
          const numberOfFrames = frame.numberOfFrames;
          const channelData: Float32Array[] = [];

          // Copy audio data from each channel
          for (let i = 0; i < numberOfChannels; i++) {
            const buffer = new Float32Array(numberOfFrames);
            frame.copyTo(buffer, { planeIndex: i });
            channelData.push(buffer);
          }

          // Store the decoded audio samples
          this.audioHandler!.audioSamples = channelData;

          // Play the decoded audio
          this.playWebCodecsAudio(channelData, frame.sampleRate, frame.timestamp);

          // Close the frame to free resources
          frame.close();

          // Decrement pending decodes counter
          this.audioHandler!.pendingDecodes--;
        } catch (e) {
          console.error("Error in output callback:", e);
          this.audioHandler!.pendingDecodes--;
        }
      };

      // Set up error handler
      const errorCallback = (error: DOMException) => {
        console.error("AudioDecoder error:", error);
        this.audioHandler!.pendingDecodes--;
      };

      // Create the audio decoder with our callbacks
      const decoder = new AudioDecoder({
        output: outputCallback,
        error: errorCallback
      });

      // Store the decoder
      this.audioHandler.webCodecsDecoder = decoder;

      console.log("WebCodecs AudioDecoder initialized");
    } catch (e) {
      console.error("Error initializing WebCodecs AudioDecoder:", e);
    }
  }

  // Play decoded audio data from WebCodecs
  private playWebCodecsAudio(channelData: Float32Array[], sampleRate: number, timestamp: number): void {
    if (!this.audioHandler?.audioContext) return;

    try {
      const audioContext = this.audioHandler.audioContext;
      const numberOfChannels = channelData.length;
      const frameCount = channelData[0].length;

      // Create an audio buffer
      const audioBuffer = audioContext.createBuffer(
        numberOfChannels,
        frameCount,
        sampleRate
      );

      // Fill the buffer with decoded data
      for (let channel = 0; channel < numberOfChannels; channel++) {
        if (channel < channelData.length) {
          const channelDataArray = audioBuffer.getChannelData(channel);
          channelDataArray.set(channelData[channel]);
        }
      }

      // Create a buffer source and play it
      const source = audioContext.createBufferSource();
      source.buffer = audioBuffer;
      source.connect(this.audioHandler.gainNode!);
      source.start();

      console.log("Playing WebCodecs decoded audio");
    } catch (e) {
      console.error("Error playing WebCodecs audio:", e);
    }
  }

  private async handleAudioFrame(frameData: any): Promise<void> {
    if (!this.audioHandler || !this.audioHandler.audioContext) {
      return;
    }

    try {
      // Get frame data
      const data = await frameData.data();
      const codec = await frameData.codec();
      const sampleRate = await frameData.sample_rate();
      const channels = await frameData.channels();

      // Store the frame for debug purposes
      if (this.audioHandler.lastFrames.length >= 20) {
        this.audioHandler.lastFrames.shift(); // Remove oldest frame
      }
      this.audioHandler.lastFrames.push(data.slice(0)); // Store a copy

      // Log frame info
      console.log(`Received audio frame: codec=${codec}, size=${data.length} bytes, sampleRate=${sampleRate}, channels=${channels}`);

      // Try to decode with WebCodecs if available
      if (this.audioHandler.webCodecsDecoder) {
        try {
          // Configure the decoder if it's not configured yet
          if (this.audioHandler.webCodecsDecoder.state === "unconfigured") {
            const config: AudioDecoderConfig = {
              codec: codec.toLowerCase(), // e.g., "opus"
              sampleRate: sampleRate,
              numberOfChannels: channels
            };

            console.log("Configuring WebCodecs AudioDecoder with:", config);
            this.audioHandler.webCodecsDecoder.configure(config);
          }

          // Create a timestamp for the frame (in microseconds)
          const timestamp = Date.now() * 1000;

          // Create an EncodedAudioChunk from the frame data
          const encodedChunk = new EncodedAudioChunk({
            type: 'key', // Opus frames are typically key frames
            timestamp: timestamp,
            duration: undefined, // Duration is optional
            data: data
          });

          // Increment pending decodes counter
          this.audioHandler.pendingDecodes++;

          // Decode the chunk
          this.audioHandler.webCodecsDecoder.decode(encodedChunk);

          console.log("Sent frame to WebCodecs decoder");
        } catch (e) {
          console.error("Error using WebCodecs decoder:", e);
          this.audioHandler.pendingDecodes--;

          // Fallback to decodeAudioData
          this.tryDecodeAudioData(data, codec, sampleRate, channels);
        }
      } else {
        // Fallback to decodeAudioData
        this.tryDecodeAudioData(data, codec, sampleRate, channels);
      }
    } catch (e) {
      console.error("Error handling audio frame:", e);
    }
  }

  // Fallback method to try browser's built-in audio decoding
  private tryDecodeAudioData(data: Uint8Array, codec: string, sampleRate: number, channels: number): void {
    if (!this.audioHandler?.audioContext) return;

    try {
      // Convert to ArrayBuffer
      const buffer = data.buffer.slice(
        data.byteOffset,
        data.byteOffset + data.byteLength
      );

      // Try to decode with native browser API
      this.audioHandler.audioContext.decodeAudioData(
        buffer,
        (audioBuffer) => {
          console.log("Successfully decoded audio buffer with decodeAudioData!");
          this.playDecodedAudio(audioBuffer);
        },
        (error) => {
          console.warn(`Failed to decode audio data (${codec}) with decodeAudioData:`, error);
        }
      );
    } catch (e) {
      console.warn(`Exception in decodeAudioData (${codec}):`, e);
    }
  }

  private playDecodedAudio(audioBuffer: AudioBuffer): void {
    if (!this.audioHandler || !this.audioHandler.audioContext) {
      return;
    }

    try {
      // Create buffer source
      const source = this.audioHandler.audioContext.createBufferSource();
      source.buffer = audioBuffer;

      // Connect to gain node for volume control
      source.connect(this.audioHandler.gainNode!);

      // Start playback
      source.start();

      console.log("Playing audio buffer:", {
        duration: audioBuffer.duration,
        sampleRate: audioBuffer.sampleRate,
        numberOfChannels: audioBuffer.numberOfChannels
      });
    } catch (e) {
      console.error("Error playing decoded audio:", e);
    }
  }
}

// Signal that we're done loading the WASM module.
postMessage("loaded");

// Technically, there's a race condition here...
Comlink.expose(new Bridge());