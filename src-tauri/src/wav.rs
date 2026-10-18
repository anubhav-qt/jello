// Writes rendered sound to WAV files.

use std::path::Path;

use crate::engine::CHANNELS;

// Writes stereo samples (left, right, left, ...) as a 32-bit float WAV file,
// the same numbers the engine made with nothing rounded off.
pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: CHANNELS as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec).map_err(|error| error.to_string())?;
    for sample in samples {
        writer.write_sample(*sample).map_err(|error| error.to_string())?;
    }
    writer.finalize().map_err(|error| error.to_string())?;

    Ok(())
}
