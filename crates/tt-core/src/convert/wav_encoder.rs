use std::io::Write;
use std::path::Path;

/// Write audio data as WAV file (PCM 16-bit or 24-bit)
pub fn write_wav(
    path: &Path,
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
) -> anyhow::Result<()> {
    let mut file = std::fs::File::create(path)?;

    let num_samples = samples.len() as u32;
    let bytes_per_sample = bits_per_sample / 8;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
    let block_align = channels * bytes_per_sample;
    let data_size = num_samples * bytes_per_sample as u32;

    // RIFF header
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_size).to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // fmt chunk
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // chunk size
    file.write_all(&1u16.to_le_bytes())?;  // PCM format
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&bits_per_sample.to_le_bytes())?;

    // data chunk
    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    // Convert f32 samples to PCM
    match bits_per_sample {
        16 => {
            let mut buf = Vec::with_capacity(num_samples as usize * 2);
            for &s in samples {
                let s16 = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                buf.extend_from_slice(&s16.to_le_bytes());
            }
            file.write_all(&buf)?;
        }
        24 => {
            let mut buf = Vec::with_capacity(num_samples as usize * 3);
            for &s in samples {
                let s24 = (s.clamp(-1.0, 1.0) * 8388607.0) as i32;
                buf.extend_from_slice(&s24.to_le_bytes()[..3]);
            }
            file.write_all(&buf)?;
        }
        _ => {
            anyhow::bail!("Unsupported bit depth: {}", bits_per_sample);
        }
    }

    file.flush()?;
    Ok(())
}
