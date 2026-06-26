use std::path::Path;

use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::buffer::AudioBuffer;
use crate::codecs::{AudioDecoder, DecoderInstance, ProbeResult};
use tt_common::SongMetadata;

/// Symphonia-based decoder (0.6): FLAC, MP3, AAC/ALAC, Vorbis/Opus, WAV, PCM
pub struct SymphoniaDecoder;

impl AudioDecoder for SymphoniaDecoder {
    fn name(&self) -> &'static str {
        "symphonia"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[
            "flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "alac", "wma",
        ]
    }
    fn priority(&self) -> u8 {
        10
    }

    fn probe(&self, magic: &[u8], extension: &str) -> ProbeResult {
        match extension {
            "flac" if magic.starts_with(b"fLaC") => ProbeResult::Match,
            "mp3" => {
                if magic.len() >= 2 && (magic[0] == 0xFF && magic[1] & 0xE0 == 0xE0) {
                    ProbeResult::Match
                } else {
                    ProbeResult::Maybe
                }
            }
            "wav" if magic.starts_with(b"RIFF") => ProbeResult::Match,
            "ogg" | "opus" if magic.starts_with(b"OggS") => ProbeResult::Match,
            "ogg" | "opus" => ProbeResult::Maybe,
            "aac" | "m4a" => ProbeResult::Maybe,
            "alac" | "m4b" => ProbeResult::Maybe,
            "wma" => {
                if magic.len() >= 16
                    && &magic[0..16]
                        == b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA\x00\x62\xCE\x6C"
                {
                    ProbeResult::Match
                } else {
                    ProbeResult::Maybe
                }
            }
            _ => ProbeResult::No,
        }
    }

    fn open(&self, path: &Path) -> anyhow::Result<Box<dyn DecoderInstance>> {
        let src = std::fs::File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(src), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        // 0.6: probe() returns Box<dyn FormatReader> directly
        let format = symphonia::default::get_probe().probe(
            &hint,
            mss,
            format_opts,
            metadata_opts,
        )?;

        // Find audio track
        let track = format
            .tracks()
            .iter()
            .find(|t| matches!(&t.codec_params, Some(CodecParameters::Audio(_))))
            .ok_or_else(|| anyhow::anyhow!("no supported audio track found"))?;

        let track_id = track.id;

        let audio_params = match &track.codec_params {
            Some(CodecParameters::Audio(a)) => a.clone(),
            _ => unreachable!(),
        };

        let sample_rate = audio_params.sample_rate.unwrap_or(44100);
        let channels = audio_params
            .channels
            .as_ref()
            .map(|c| c.count() as u16)
            .unwrap_or(2);

        // Compute the total duration in milliseconds.
        //
        // Symphonia 0.6's `AudioCodecParameters` no longer exposes a total
        // frame count directly (the old `n_frames` was removed), so we derive
        // the duration from the container-level track/media metadata instead.
        //
        // Priority:
        //   1. `track.num_frames` / `sample_rate` — for formats whose container
        //      stores an exact sample/frame count (FLAC STREAMINFO, Ogg, etc.).
        //      `num_frames` is the number of *playable* audio frames, so for
        //      PCM-like codecs this is the total sample count per channel.
        //   2. `track.duration` converted via `track.time_base` — for formats
        //      that store a duration in timebase ticks (MP3/Xing, MP4, etc.).
        //   3. `media_info().duration` converted via `media_info().time_base` —
        //      container-level aggregate duration as a last resort.
        //   4. `None` — duration unknown until full decode.
        //
        // The previous code used `max_frames_per_packet` (samples *per packet*,
        // ~4096 for FLAC ≈ 93ms) which produced a near-zero duration and caused
        // FLAC files to show no duration in the UI (see tt-core eval report).
        let duration = (|| -> Option<u64> {
            // 1. num_frames / sample_rate
            if let Some(n) = track.num_frames {
                if n > 0 && sample_rate > 0 {
                    return Some(n * 1000 / sample_rate as u64);
                }
            }
            // 2. track.duration + track.time_base
            if let (Some(tb), Some(dur)) = (track.time_base, track.duration) {
                let ts = symphonia::core::units::Timestamp::new(dur.get() as i64);
                let time = tb.calc_time_saturating(ts);
                return Some((time.as_secs_f64() * 1000.0) as u64);
            }
            // 3. media_info duration + time_base
            let mi = format.media_info();
            if let (Some(tb), Some(dur)) = (mi.time_base, mi.duration) {
                let ts = symphonia::core::units::Timestamp::new(dur.get() as i64);
                let time = tb.calc_time_saturating(ts);
                return Some((time.as_secs_f64() * 1000.0) as u64);
            }
            None
        })();

        tracing::debug!(
            "symphonia open: codec={:?} sr={} ch={} num_frames={:?} track_dur={:?} media_dur={:?} -> duration_ms={:?}",
            audio_params.codec, sample_rate, channels,
            track.num_frames, track.duration, format.media_info().duration, duration,
        );

        let decoder_opts = AudioDecoderOptions::default();
        let decoder =
            symphonia::default::get_codecs().make_audio_decoder(&audio_params, &decoder_opts)?;

        Ok(Box::new(SymphoniaInstance {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            duration_ms: duration,
            metadata: None,
        }))
    }
}

struct SymphoniaInstance {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::audio::AudioDecoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    duration_ms: Option<u64>,
    metadata: Option<SongMetadata>,
}

impl DecoderInstance for SymphoniaInstance {
    fn decode(&mut self) -> anyhow::Result<Option<AudioBuffer>> {
        // 0.6: next_packet() returns Result<Option<Packet>>
        let packet = match self.format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => return Ok(None),
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id != self.track_id {
            return self.decode();
        }

        let decoded: GenericAudioBufferRef<'_> = self.decoder.decode(&packet)?;
        let spec = decoded.spec().clone();
        let channels = spec.channels().count() as u16;
        let num_frames = decoded.frames();

        if num_frames == 0 {
            return Ok(Some(AudioBuffer::new(channels, 0, spec.rate())));
        }

        let mut interleaved = vec![0.0f32; channels as usize * num_frames];
        decoded.copy_to_slice_interleaved::<f32, _>(interleaved.as_mut_slice());

        Ok(Some(AudioBuffer::from_interleaved(
            &interleaved,
            channels,
            spec.rate(),
        )))
    }

    fn seek(&mut self, frame: u64) -> anyhow::Result<()> {
        use symphonia::core::formats::{SeekMode, SeekTo};
        use symphonia::core::units::Time;

        // Symphonia 0.6's FormatReader seeks by Time/Timestamp, not by sample
        // frame. The `frame` argument coming from the player is a sample-frame
        // offset relative to `sample_rate`, so convert it into a Time. For
        // reasonable audio lengths this f64 conversion is always finite and
        // within i64 range; fall back to ZERO if it ever isn't.
        let sample_rate = self.sample_rate.max(1) as f64;
        let secs = frame as f64 / sample_rate;
        let time = Time::try_from_secs_f64(secs).unwrap_or(Time::ZERO);

        let seeked = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;

        // Per symphonia docs, the decoder must be reset after a seek so that
        // any internally buffered samples from before the seek point are
        // flushed; otherwise the next decode() would emit stale audio.
        self.decoder.reset();

        tracing::debug!(
            "symphonia seek: requested frame {} (~{:.3}s), landed on track {}",
            frame,
            secs,
            seeked.track_id
        );
        Ok(())
    }

    fn total_frames(&self) -> Option<u64> {
        None
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn metadata(&self) -> Option<SongMetadata> {
        self.metadata.clone()
    }
    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }
}
