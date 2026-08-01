# Audio Pipeline

The model always receives one controlled stereo Float32 PCM WAV at 44,100 Hz. Rust invokes the
pinned FFmpeg executable with an argument array and uses SoXR with precision 32. Float
intermediates are never dithered.

## Compatibility mode

1. Decode the source once to the controlled model-input WAV.
2. Run the worker and obtain a 44.1 kHz stereo Float32 vocals WAV.
3. Subtract vocals from that exact model-input WAV in the float domain with normalization disabled.
4. Encode to the selected output format, validate with FFprobe, and publish the `.partial` file by
   rename.

Reusing the exact model input avoids a second decode/resample path and is why compatibility mode is
the default.

## Experimental source-sample-rate mode

1. Decode the original source to stereo Float32 at its native sample rate.
2. Resample the 44.1 kHz vocals to the native sample rate with SoXR precision 32.
3. Subtract in Float32 using the decoded source as duration authority.
4. Encode at the source sample rate and preserve common source bit depth when the selected codec
   supports it.

The experimental label remains until measured alignment behavior is implemented across relevant
codecs and sample-rate pairs.

## Dither and publication

Float32 WAV output is not dithered. When Float32 is quantized to integer PCM, triangular dither is
applied exactly once during final encoding. Final files are written as `.partial` in the selected
output directory, inspected with FFprobe, and renamed only after validation succeeds.

## Deferred audio work

- `AUDIO-001`: measure decoder, resampler, framing, and reverse-resampler offsets before adding
  source-rate alignment correction.
- `AUDIO-002`: define exact decoded-sample-count trim and pad behavior.
- `AUDIO-003`: measure over-range residual peaks and choose an explicit clipping or limiting policy.
- `AUDIO-004`: define safe metadata and cover-art copying.
- `AUDIO-005`: measure whether a later mode can restore mono output without discarding useful
  channel-different estimates.
