use std::{
    ffi::{OsStr, OsString},
    fs,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, de::Error as _};
use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

use crate::{
    domain::{AppError, ConflictPolicy, ErrorCode, OutputFormat},
    runtime::AppPaths,
};

pub const MODEL_SAMPLE_RATE: u32 = 44_100;
const SOXR_FILTER: &str = "aresample=resampler=soxr:osr=44100:precision=32";
const RESIDUAL_FILTER: &str = "[1:a:0]volume=-1[negative_vocals];[0:a:0][negative_vocals]amix=inputs=2:duration=first:dropout_transition=0:normalize=0[out]";

#[derive(Clone, Debug, Deserialize)]
pub struct FFprobeDocument {
    pub streams: Vec<FFprobeStream>,
    pub format: Option<FFprobeFormat>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct FFprobeStream {
    pub index: u32,
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub sample_rate: Option<String>,
    pub channels: Option<u8>,
    pub channel_layout: Option<String>,
    pub duration: Option<String>,
    pub sample_fmt: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_u8")]
    pub bits_per_sample: Option<u8>,
    #[serde(default, deserialize_with = "deserialize_optional_u8")]
    pub bits_per_raw_sample: Option<u8>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FFprobeU8 {
    Number(u8),
    String(String),
}

fn deserialize_optional_u8<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<FFprobeU8>::deserialize(deserializer)? {
        None => Ok(None),
        Some(FFprobeU8::Number(value)) => Ok(Some(value)),
        Some(FFprobeU8::String(value)) if value == "N/A" || value.is_empty() => Ok(None),
        Some(FFprobeU8::String(value)) => value.parse::<u8>().map(Some).map_err(D::Error::custom),
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct FFprobeFormat {
    pub format_name: Option<String>,
    pub duration: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AudioSourceInfo {
    pub stream_index: u32,
    pub codec_name: String,
    pub sample_rate: u32,
    pub channels: u8,
    pub duration_seconds: f64,
    pub sample_format: Option<String>,
    pub bits_per_sample: Option<u8>,
    pub bits_per_raw_sample: Option<u8>,
    pub container_format: Option<String>,
}

pub fn parse_source_info(json: &str) -> Result<AudioSourceInfo, AppError> {
    let document: FFprobeDocument = serde_json::from_str(json).map_err(|_| {
        AppError::new(
            ErrorCode::InputUnsupported,
            "FFprobe did not return valid JSON",
        )
    })?;
    let stream = document
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("audio"))
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::InputUnsupported,
                "input contains no audio stream",
            )
        })?;
    let channels = stream.channels.ok_or_else(|| {
        AppError::new(
            ErrorCode::InputUnsupported,
            "audio channel count is missing",
        )
    })?;
    if !(1..=2).contains(&channels) {
        return Err(AppError::new(
            ErrorCode::InputUnsupported,
            "MVP supports one or two audio channels",
        ));
    }
    let sample_rate = stream
        .sample_rate
        .as_deref()
        .and_then(|value| value.parse().ok())
        .filter(|value: &u32| *value > 0)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::InputUnsupported,
                "audio sample rate is missing or invalid",
            )
        })?;
    let duration_seconds = stream
        .duration
        .as_deref()
        .or(document
            .format
            .as_ref()
            .and_then(|format| format.duration.as_deref()))
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.01)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::InputUnsupported,
                "audio duration is missing or too short",
            )
        })?;
    Ok(AudioSourceInfo {
        stream_index: stream.index,
        codec_name: stream.codec_name.clone().unwrap_or_default(),
        sample_rate,
        channels,
        duration_seconds,
        sample_format: stream.sample_fmt.clone(),
        bits_per_sample: stream.bits_per_sample,
        bits_per_raw_sample: stream.bits_per_raw_sample,
        container_format: document.format.and_then(|format| format.format_name),
    })
}

pub fn is_supported_input(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("wav" | "flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "aiff" | "aif" | "wma")
    )
}

pub fn model_input_args(
    source: &Path,
    source_info: &AudioSourceInfo,
    output: &Path,
) -> Vec<OsString> {
    vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-i".into(),
        source.as_os_str().into(),
        "-map".into(),
        format!("0:{}", source_info.stream_index).into(),
        "-vn".into(),
        "-af".into(),
        SOXR_FILTER.into(),
        "-ac".into(),
        "2".into(),
        "-c:a".into(),
        "pcm_f32le".into(),
        output.as_os_str().into(),
    ]
}

pub fn compatibility_residual_args(
    model_input: &Path,
    vocals: &Path,
    output: &Path,
    format: OutputFormat,
    source_info: &AudioSourceInfo,
) -> Vec<OsString> {
    let filter = residual_filter(RESIDUAL_FILTER, format, source_info);
    let mut args = base_residual_args(model_input, vocals, &filter);
    args.extend(final_encoding_args(format, MODEL_SAMPLE_RATE, source_info));
    args.push(output.as_os_str().into());
    args
}

pub fn source_native_args(
    source: &Path,
    source_info: &AudioSourceInfo,
    output: &Path,
) -> Vec<OsString> {
    vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-i".into(),
        source.as_os_str().into(),
        "-map".into(),
        format!("0:{}", source_info.stream_index).into(),
        "-vn".into(),
        "-ac".into(),
        "2".into(),
        "-ar".into(),
        source_info.sample_rate.to_string().into(),
        "-c:a".into(),
        "pcm_f32le".into(),
        output.as_os_str().into(),
    ]
}

pub fn source_rate_residual_args(
    source_native: &Path,
    vocals: &Path,
    output: &Path,
    format: OutputFormat,
    source_info: &AudioSourceInfo,
) -> Vec<OsString> {
    let filter = residual_filter(
        &format!(
            "[1:a:0]aresample=resampler=soxr:osr={}:precision=32[vocals];[vocals]volume=-1[negative_vocals];[0:a:0][negative_vocals]amix=inputs=2:duration=first:dropout_transition=0:normalize=0[out]",
            source_info.sample_rate
        ),
        format,
        source_info,
    );
    let mut args = base_residual_args(source_native, vocals, &filter);
    args.extend(final_encoding_args(
        format,
        source_info.sample_rate,
        source_info,
    ));
    args.push(output.as_os_str().into());
    args
}

fn base_residual_args(mixture: &Path, vocals: &Path, filter: &str) -> Vec<OsString> {
    vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-i".into(),
        mixture.as_os_str().into(),
        "-i".into(),
        vocals.as_os_str().into(),
        "-filter_complex".into(),
        filter.into(),
        "-map".into(),
        "[out]".into(),
        "-ac".into(),
        "2".into(),
    ]
}
fn residual_filter(base: &str, format: OutputFormat, source: &AudioSourceInfo) -> String {
    match format {
        OutputFormat::WavFloat32 => base.into(),
        OutputFormat::Flac => {
            let sample_fmt = if flac_bits(source) == 16 {
                "s16"
            } else {
                "s32"
            };
            base.replacen("[out]", "[mixed]", 1)
                + &format!(";[mixed]aresample=osf={sample_fmt}:dither_method=triangular[out]")
        }
    }
}
fn final_encoding_args(
    format: OutputFormat,
    sample_rate: u32,
    source: &AudioSourceInfo,
) -> Vec<OsString> {
    match format {
        OutputFormat::WavFloat32 => vec![
            "-ar".into(),
            sample_rate.to_string().into(),
            "-c:a".into(),
            "pcm_f32le".into(),
            "-f".into(),
            "wav".into(),
        ],
        OutputFormat::Flac => {
            let bits = flac_bits(source);
            vec![
                "-ar".into(),
                sample_rate.to_string().into(),
                "-c:a".into(),
                "flac".into(),
                "-sample_fmt".into(),
                if flac_bits(source) == 16 {
                    "s16"
                } else {
                    "s32"
                }
                .into(),
                "-bits_per_raw_sample".into(),
                bits.to_string().into(),
                "-output_sample_bits".into(),
                bits.to_string().into(),
                "-f".into(),
                "flac".into(),
            ]
        }
    }
}
fn flac_bits(source: &AudioSourceInfo) -> u8 {
    match source.bits_per_raw_sample.or(source.bits_per_sample) {
        Some(16) => 16,
        Some(24) => 24,
        _ => 24,
    }
}

#[derive(Clone, Debug)]
pub struct JobPaths {
    pub root: PathBuf,
    pub model_input: PathBuf,
    pub vocals: PathBuf,
    pub source_native: PathBuf,
    pub logs: PathBuf,
}
pub fn create_job_paths(
    app_paths: &AppPaths,
    task_id: &str,
    item_id: &str,
) -> Result<JobPaths, AppError> {
    if !safe_id(task_id) || !safe_id(item_id) {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "task and item identifiers must be filesystem-safe",
        ));
    }
    let root = app_paths.jobs().join(task_id).join(item_id);
    fs::create_dir_all(root.join("input")).map_err(audio_error)?;
    fs::create_dir_all(root.join("model")).map_err(audio_error)?;
    fs::create_dir_all(root.join("native")).map_err(audio_error)?;
    let logs = root.join("logs");
    fs::create_dir_all(&logs).map_err(audio_error)?;
    Ok(JobPaths {
        model_input: root.join("input").join("model-input.wav"),
        vocals: root.join("model").join("vocals.wav"),
        source_native: root.join("native").join("source-native.wav"),
        logs,
        root,
    })
}
pub fn cleanup_job(app_paths: &AppPaths, job: &JobPaths) -> Result<(), AppError> {
    if !job.root.starts_with(app_paths.jobs()) {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "refusing to clean a job outside the private jobs root",
        ));
    }
    if job.root.exists() {
        fs::remove_dir_all(&job.root).map_err(audio_error)?;
    }
    Ok(())
}
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[derive(Clone, Debug)]
pub struct OutputPlan {
    pub final_path: PathBuf,
    pub partial_path: PathBuf,
    pub skip: bool,
}
pub fn plan_output(
    output_dir: &Path,
    source: &Path,
    task_id: &str,
    format: OutputFormat,
    conflict: ConflictPolicy,
) -> Result<OutputPlan, AppError> {
    let stem = source
        .file_stem()
        .and_then(OsStr::to_str)
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| AppError::new(ErrorCode::InvalidRequest, "source file has no valid name"))?;
    let extension = match format {
        OutputFormat::Flac => "flac",
        OutputFormat::WavFloat32 => "wav",
    };
    let base = format!("{stem} (Instrumental)");
    let mut final_path = output_dir.join(format!("{base}.{extension}"));
    match conflict {
        ConflictPolicy::Skip if final_path.exists() => {
            return Ok(OutputPlan {
                partial_path: partial_path(output_dir, stem, task_id, extension),
                final_path,
                skip: true,
            });
        }
        ConflictPolicy::AutoNumber => {
            let mut index = 2;
            while final_path.exists() {
                final_path = output_dir.join(format!("{base} ({index}).{extension}"));
                index += 1;
            }
        }
        _ => {}
    }
    if final_path == source {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "output path matches source path",
        ));
    }
    Ok(OutputPlan {
        partial_path: partial_path(output_dir, stem, task_id, extension),
        final_path,
        skip: false,
    })
}
fn partial_path(output_dir: &Path, stem: &str, task_id: &str, extension: &str) -> PathBuf {
    output_dir.join(format!(".{stem}.{task_id}.partial.{extension}"))
}

pub fn validate_output(info: &AudioSourceInfo, expected_sample_rate: u32) -> Result<(), AppError> {
    if info.channels != 2
        || info.sample_rate != expected_sample_rate
        || info.duration_seconds < 0.01
    {
        return Err(AppError::new(
            ErrorCode::PostprocessFailed,
            "final audio output properties are invalid",
        ));
    }
    Ok(())
}
pub fn publish_partial(partial: &Path, final_path: &Path, overwrite: bool) -> Result<(), AppError> {
    if !partial.exists() {
        return Err(AppError::new(
            ErrorCode::PostprocessFailed,
            "partial output does not exist",
        ));
    }
    if final_path.exists() {
        if !overwrite {
            return Err(AppError::new(
                ErrorCode::OutputNotWritable,
                "output appeared after planning",
            ));
        }
        replace_file(partial, final_path)?;
    } else {
        fs::rename(partial, final_path).map_err(audio_error)?;
    }
    Ok(())
}
pub fn discard_partial(partial: &Path) -> Result<(), AppError> {
    if partial.exists() {
        fs::remove_file(partial).map_err(audio_error)?;
    }
    Ok(())
}
fn replace_file(partial: &Path, final_path: &Path) -> Result<(), AppError> {
    let replacement = wide_path(partial);
    let replaced = wide_path(final_path);
    let result = unsafe {
        ReplaceFileW(
            replaced.as_ptr(),
            replacement.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result == 0 {
        return Err(AppError::new(
            ErrorCode::PostprocessFailed,
            "could not atomically replace output file",
        ));
    }
    Ok(())
}
fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
fn audio_error(_: impl std::fmt::Display) -> AppError {
    AppError::new(
        ErrorCode::PostprocessFailed,
        "audio filesystem operation failed",
    )
}

// AUDIO-001: source-rate alignment needs measured offset compensation.
// AUDIO-002: source sample-count trim/pad policy remains to be measured.
// AUDIO-003: residual clipping policy remains intentionally undecided.

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        process::Command,
    };
    use uuid::Uuid;
    fn source_info() -> AudioSourceInfo {
        AudioSourceInfo {
            stream_index: 2,
            codec_name: "flac".into(),
            sample_rate: 48_000,
            channels: 2,
            duration_seconds: 3.0,
            sample_format: Some("s32".into()),
            bits_per_sample: Some(24),
            bits_per_raw_sample: Some(24),
            container_format: Some("flac".into()),
        }
    }
    fn strings(values: Vec<OsString>) -> Vec<String> {
        values
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }
    #[test]
    fn parses_first_audio_stream_and_rejects_three_channels() {
        let json = r#"{"streams":[{"index":0,"codec_type":"video"},{"index":1,"codec_type":"audio","codec_name":"flac","sample_rate":"48000","channels":2,"duration":"1.5","sample_fmt":"s32","bits_per_sample":0,"bits_per_raw_sample":"24"}],"format":{"format_name":"flac"}}"#;
        let parsed = parse_source_info(json).unwrap();
        assert_eq!(parsed.stream_index, 1);
        assert_eq!(parsed.bits_per_sample, Some(0));
        assert_eq!(parsed.bits_per_raw_sample, Some(24));
        let invalid = json.replace("\"channels\":2", "\"channels\":3");
        assert!(parse_source_info(&invalid).is_err());
        let invalid_depth = json.replace("\"24\"", "\"not-a-bit-depth\"");
        assert!(parse_source_info(&invalid_depth).is_err());
    }
    #[test]
    fn model_input_has_exact_soxr_float_vector() {
        let args = strings(model_input_args(
            Path::new("input.flac"),
            &source_info(),
            Path::new("model.wav"),
        ));
        assert_eq!(
            args,
            vec![
                "-hide_banner",
                "-nostdin",
                "-y",
                "-i",
                "input.flac",
                "-map",
                "0:2",
                "-vn",
                "-af",
                "aresample=resampler=soxr:osr=44100:precision=32",
                "-ac",
                "2",
                "-c:a",
                "pcm_f32le",
                "model.wav"
            ]
        );
    }
    #[test]
    fn compatibility_filter_negates_vocals_before_mixing() {
        let args = strings(compatibility_residual_args(
            Path::new("model.wav"),
            Path::new("vocals.wav"),
            Path::new("out.flac"),
            OutputFormat::Flac,
            &source_info(),
        ));
        assert!(
            args.iter()
                .any(|value| value.contains("[1:a:0]volume=-1[negative_vocals]"))
        );
        assert!(args.iter().any(|value| value.contains(
            "[0:a:0][negative_vocals]amix=inputs=2:duration=first:dropout_transition=0:normalize=0"
        )));
        assert!(!args.iter().any(|value| value.contains("weights=1 -1")));
        assert!(
            args.iter()
                .any(|value| value.contains("dither_method=triangular"))
        );
        assert!(!args.contains(&"-af".into()));
    }
    #[test]
    fn float_wav_has_no_dither() {
        let args = strings(compatibility_residual_args(
            Path::new("model.wav"),
            Path::new("vocals.wav"),
            Path::new("out.wav"),
            OutputFormat::WavFloat32,
            &source_info(),
        ));
        assert!(!args.iter().any(|value| value.contains("dither")));
        assert!(args.contains(&"pcm_f32le".into()));
        assert!(!args.iter().any(|value| value == "-output_sample_bits"));
    }
    #[test]
    fn flac_dither_targets_selected_output_bits_once() {
        for (bits, sample_fmt) in [(16, "s16"), (24, "s32")] {
            let mut info = source_info();
            info.bits_per_sample = Some(bits);
            info.bits_per_raw_sample = Some(bits);
            let args = strings(compatibility_residual_args(
                Path::new("model.wav"),
                Path::new("vocals.wav"),
                Path::new("out.flac"),
                OutputFormat::Flac,
                &info,
            ));
            assert_eq!(
                args.iter()
                    .filter(|value| value.contains("dither_method=triangular"))
                    .count(),
                1
            );
            assert!(
                args.iter()
                    .any(|value| value.contains(&format!("osf={sample_fmt}")))
            );
            assert_eq!(
                args.iter()
                    .filter(|value| value.as_str() == "-output_sample_bits")
                    .count(),
                1
            );
            let index = args
                .iter()
                .position(|value| value == "-output_sample_bits")
                .unwrap();
            assert_eq!(args[index + 1], bits.to_string());
            let raw = args
                .iter()
                .position(|value| value == "-bits_per_raw_sample")
                .unwrap();
            assert_eq!(args[raw + 1], bits.to_string());
        }
    }
    #[test]
    fn source_rate_residual_uses_source_rate_and_soxr() {
        let args = strings(source_rate_residual_args(
            Path::new("source.wav"),
            Path::new("vocals.wav"),
            Path::new("out.wav"),
            OutputFormat::WavFloat32,
            &source_info(),
        ));
        assert!(
            args.iter()
                .any(|value| value.contains("osr=48000:precision=32"))
        );
        assert!(
            args.iter()
                .any(|value| value.contains("[vocals]volume=-1[negative_vocals]"))
        );
        assert!(!args.iter().any(|value| value.contains("weights=1 -1")));
        assert!(args.contains(&"-ar".into()));
    }
    #[test]
    fn auto_numbering_keeps_media_extension_on_partial_output() {
        let directory = std::env::temp_dir().join(format!("soufmer-audio-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("song (Instrumental).flac"), b"existing").unwrap();
        let plan = plan_output(
            &directory,
            Path::new("song.flac"),
            "task-1",
            OutputFormat::Flac,
            ConflictPolicy::AutoNumber,
        )
        .unwrap();
        assert_eq!(
            plan.final_path.file_name().unwrap(),
            "song (Instrumental) (2).flac"
        );
        assert!(
            plan.partial_path
                .to_string_lossy()
                .ends_with(".partial.flac")
        );
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    #[cfg(windows)]
    fn generated_residual_identity_and_source_rate() {
        let discover = |name: &str, variable: &str| {
            std::env::var_os(variable).map(PathBuf::from).or_else(|| {
                Command::new("where")
                    .arg(name)
                    .output()
                    .ok()
                    .filter(|output| output.status.success())
                    .and_then(|output| {
                        String::from_utf8(output.stdout)
                            .ok()?
                            .lines()
                            .next()
                            .map(PathBuf::from)
                    })
            })
        };
        let Some(ffmpeg) = discover("ffmpeg", "SOUFMER_TEST_FFMPEG") else {
            eprintln!("skipping generated audio test: ffmpeg unavailable");
            return;
        };
        let Some(ffprobe) = discover("ffprobe", "SOUFMER_TEST_FFPROBE") else {
            eprintln!("skipping generated audio test: ffprobe unavailable");
            return;
        };
        let root = std::env::temp_dir().join(format!("soufmer-audio-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.wav");
        assert!(
            Command::new(&ffmpeg)
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:sample_rate=48000:duration=0.1",
                    "-ac",
                    "2",
                    "-c:a",
                    "pcm_f32le"
                ])
                .arg(&source)
                .status()
                .unwrap()
                .success()
        );
        let info = AudioSourceInfo {
            stream_index: 0,
            codec_name: "pcm_f32le".into(),
            sample_rate: 48000,
            channels: 2,
            duration_seconds: 0.1,
            sample_format: Some("flt".into()),
            bits_per_sample: None,
            bits_per_raw_sample: None,
            container_format: Some("wav".into()),
        };
        let model = root.join("model.wav");
        assert!(
            Command::new(&ffmpeg)
                .args(model_input_args(&source, &info, &model))
                .status()
                .unwrap()
                .success()
        );
        let zero = root.join("zero.wav");
        assert!(
            Command::new(&ffmpeg)
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "anullsrc=r=44100:cl=stereo",
                    "-t",
                    "0.1",
                    "-c:a",
                    "pcm_f32le"
                ])
                .arg(&zero)
                .status()
                .unwrap()
                .success()
        );
        let identity = root.join("identity.wav");
        assert!(
            Command::new(&ffmpeg)
                .args(compatibility_residual_args(
                    &model,
                    &zero,
                    &identity,
                    OutputFormat::WavFloat32,
                    &info
                ))
                .status()
                .unwrap()
                .success()
        );
        let decode = |path: &Path| {
            Command::new(&ffmpeg)
                .args(["-v", "error", "-i"])
                .arg(path)
                .args(["-f", "f32le", "-"])
                .output()
                .unwrap()
                .stdout
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>()
        };
        let model_samples = decode(&model);
        let result = decode(&identity);
        assert_eq!(model_samples.len(), result.len());
        assert!(
            model_samples
                .iter()
                .zip(result)
                .map(|(a, b)| (a - b).abs())
                .fold(0_f32, f32::max)
                < 1e-5
        );
        let silent = root.join("silent.wav");
        assert!(
            Command::new(&ffmpeg)
                .args(compatibility_residual_args(
                    &model,
                    &model,
                    &silent,
                    OutputFormat::WavFloat32,
                    &info
                ))
                .status()
                .unwrap()
                .success()
        );
        assert!(
            decode(&silent)
                .iter()
                .map(|value| value.abs())
                .fold(0_f32, f32::max)
                < 1e-5
        );
        let probe = |path: &Path| {
            let output = Command::new(&ffprobe)
                .args([
                    "-v",
                    "error",
                    "-show_streams",
                    "-show_format",
                    "-of",
                    "json",
                ])
                .arg(path)
                .output()
                .unwrap();
            parse_source_info(std::str::from_utf8(&output.stdout).unwrap()).unwrap()
        };
        let inspected = probe(&identity);
        assert_eq!(
            (
                inspected.sample_rate,
                inspected.channels,
                inspected.codec_name.as_str(),
                inspected.sample_format.as_deref()
            ),
            (44_100, 2, "pcm_f32le", Some("flt"))
        );
        for bits in [16_u8, 24] {
            let mut flac_info = info.clone();
            flac_info.bits_per_sample = Some(bits);
            flac_info.bits_per_raw_sample = Some(bits);
            let flac = root.join(format!("instrumental-{bits}.flac"));
            assert!(
                Command::new(&ffmpeg)
                    .args(compatibility_residual_args(
                        &model,
                        &zero,
                        &flac,
                        OutputFormat::Flac,
                        &flac_info
                    ))
                    .status()
                    .unwrap()
                    .success()
            );
            let inspected = probe(&flac);
            assert_eq!(
                (
                    inspected.codec_name.as_str(),
                    inspected.sample_rate,
                    inspected.channels
                ),
                ("flac", 44_100, 2)
            );
            assert_eq!(
                inspected.bits_per_raw_sample.or(inspected.bits_per_sample),
                Some(bits)
            );
        }
        let native = root.join("native.wav");
        assert!(
            Command::new(&ffmpeg)
                .args(source_native_args(&source, &info, &native))
                .status()
                .unwrap()
                .success()
        );
        let output = root.join("out.wav");
        assert!(
            Command::new(&ffmpeg)
                .args(source_rate_residual_args(
                    &native,
                    &zero,
                    &output,
                    OutputFormat::WavFloat32,
                    &info
                ))
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(probe(&output).sample_rate, 48_000);
        let source96 = root.join("source96.wav");
        assert!(
            Command::new(&ffmpeg)
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:sample_rate=96000:duration=0.1",
                    "-ac",
                    "2",
                    "-c:a",
                    "pcm_f32le"
                ])
                .arg(&source96)
                .status()
                .unwrap()
                .success()
        );
        let info96 = AudioSourceInfo {
            sample_rate: 96000,
            ..info.clone()
        };
        let native96 = root.join("native96.wav");
        assert!(
            Command::new(&ffmpeg)
                .args(source_native_args(&source96, &info96, &native96))
                .status()
                .unwrap()
                .success()
        );
        let output96 = root.join("out96.wav");
        assert!(
            Command::new(&ffmpeg)
                .args(source_rate_residual_args(
                    &native96,
                    &zero,
                    &output96,
                    OutputFormat::WavFloat32,
                    &info96
                ))
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(probe(&output96).sample_rate, 96_000);
        fs::remove_dir_all(root).unwrap();
    }
}
