use artem::convert;
use crossterm::{
    cursor::{self},
    terminal::{self, Clear, ClearType},
    QueueableCommand,
};
use image::{DynamicImage, ImageBuffer};
use std::io::{stdout, BufReader, Read, Write};
use std::num::NonZeroU32;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const FPS: u64 = 24;
const DURATION: u64 = 3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (video_width, video_height) = get_video_dimensions("input.mp4")?;
    let (term_width, term_height) = terminal::size()?;
    let target_size = calculate_target_size(term_width, term_height);

    let frames = extract_frames(video_width, video_height)?;
    let ascii_frames: Vec<Vec<String>> = frames
        .into_iter()
        .map(|frame| frame_to_ascii(frame, target_size))
        .collect();

    let top = get_vertical_padding(&ascii_frames);
    let left = get_horizontal_padding(&ascii_frames[0]);

    let mut stdout = stdout();
    stdout.queue(Clear(ClearType::All))?.queue(cursor::Hide)?;

    let start = Instant::now();
    let frame_duration = Duration::from_millis(1000 / FPS);

    let mut output_buffer = String::new();

    for frame in ascii_frames {
        output_buffer.clear();

        for (row, line) in frame.iter().enumerate() {
            output_buffer.push_str(&format!("\x1B[{};{}H{}\n", top + row as u16, left, line));
        }

        stdout.write_all(output_buffer.as_bytes())?;
        stdout.flush()?;

        let elapsed = start.elapsed();
        let wait_time = frame_duration.saturating_sub(Duration::from_millis(
            elapsed.as_millis() as u64 % frame_duration.as_millis() as u64,
        ));

        sleep(wait_time);
    }

    stdout.queue(cursor::Show)?;
    Ok(())
}

fn get_video_dimensions(input: &str) -> Result<(u32, u32), Box<dyn std::error::Error>> {
    let output = Command::new("ffprobe")
        .args(&[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_packets",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
            input,
        ])
        .output()?;

    let output_str = String::from_utf8(output.stdout)?;
    let dimensions: Vec<u32> = output_str
        .trim()
        .split(',')
        .map(|s| s.parse().unwrap())
        .collect();

    Ok((dimensions[0], dimensions[1]))
}

fn calculate_target_size(term_width: u16, term_height: u16) -> NonZeroU32 {
    let horizontal_scale_factor = 2;
    let target = std::cmp::min(
        term_width as u32 * horizontal_scale_factor,
        term_height as u32 * 8 / 10,
    );
    NonZeroU32::new(target * 4).unwrap_or(NonZeroU32::new(80).unwrap())
}

fn extract_frames(
    width: u32,
    height: u32,
) -> Result<Vec<DynamicImage>, Box<dyn std::error::Error>> {
    let mut frames = Vec::new();
    let mut child = Command::new("ffmpeg")
        .args(&[
            "-i",
            "input.mp4",
            "-t",
            &DURATION.to_string(),
            "-f",
            "image2pipe",
            "-pix_fmt",
            "rgb24",
            "-vcodec",
            "rawvideo",
            "-",
        ])
        .stdout(Stdio::piped())
        .spawn()?;

    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut buffer = vec![0u8; (width * height * 3) as usize];

    while reader.read_exact(&mut buffer).is_ok() {
        let image_buffer = ImageBuffer::from_raw(width, height, buffer.clone())
            .ok_or("Failed to create image from buffer")?;
        frames.push(DynamicImage::ImageRgb8(image_buffer));
    }

    Ok(frames)
}

fn frame_to_ascii(frame: DynamicImage, target_size: NonZeroU32) -> Vec<String> {
    let config = artem::config::ConfigBuilder::new()
        .target_size(target_size)
        .build();
    convert(frame, &config).lines().map(String::from).collect()
}

fn get_vertical_padding(frames: &[Vec<String>]) -> u16 {
    let (_, term_height) = terminal::size().unwrap();
    let frame_height = frames[0].len();

    if frame_height < term_height as usize {
        (term_height - frame_height as u16) / 2
    } else {
        0
    }
}

fn remove_ansi_escape_sequences(input: &str) -> String {
    let mut result = String::new();
    let mut in_escape_sequence = false;

    for c in input.chars() {
        if c == '\u{1b}' {
            in_escape_sequence = true;
        } else if in_escape_sequence {
            if c.is_ascii_alphabetic() {
                in_escape_sequence = false;
            }
            continue;
        }
        result.push(c);
    }

    result
}

fn get_horizontal_padding(frame: &[String]) -> u16 {
    let (term_width, _) = terminal::size().unwrap();

    let max_line_width = frame
        .iter()
        .filter_map(|line| {
            let binding = remove_ansi_escape_sequences(line);
            let trimmed_line = binding.trim();
            if trimmed_line.is_empty() {
                None
            } else {
                Some(trimmed_line.len())
            }
        })
        .max()
        .unwrap_or(0)
        / 3;
    if max_line_width < term_width as usize {
        (term_width - max_line_width as u16) / 2
    } else {
        0
    }
}
