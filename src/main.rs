use artem::convert;
use core::str;
use crossterm::QueueableCommand;
use crossterm::cursor::{self};
use crossterm::terminal::{self, Clear, ClearType};
use image::{DynamicImage, ImageBuffer};
use std::io::{BufReader, Read, Write, stdout};
use std::num::NonZeroU32;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const INPUT: &str = "input.mp4";
const OUTPUT_FPS: u64 = 24;
const DURATION: u64 = 90;
const TARGET_SIZE: Option<NonZeroU32> = NonZeroU32::new(160);

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let (video_width, video_height) = get_video_dimensions(INPUT)?;

	let frames = extract_frames(video_width, video_height)?;
	let ascii_frames: Vec<Vec<String>> = frames
		.into_iter()
		.map(|frame| frame_to_ascii(frame, TARGET_SIZE.expect("Invalid TARGET_SIZE definition")))
		.collect();

	let top = get_vertical_padding(&ascii_frames);
	let left = get_horizontal_padding(&ascii_frames[0]);

	let mut stdout = stdout();
	stdout.queue(Clear(ClearType::All))?.queue(cursor::Hide)?;

	let frame_duration = Duration::from_secs_f64(1.0 / OUTPUT_FPS as f64);
	let start_time = Instant::now();
	let end_time = start_time + Duration::from_secs(DURATION);

	let mut frame_index = 0;
	let total_frames = ascii_frames.len();

	let mut previous_frame: Option<&Vec<String>> = None;

	while Instant::now() < end_time {
		let frame_start = Instant::now();
		let current_frame = &ascii_frames[frame_index];

		if let Some(previous) = &previous_frame {
			for (row, line) in current_frame.iter().enumerate() {
				if let Some(previous_line) = previous.get(row) {
					if previous_line != line {
						let cursor_move = format!("\x1B[{};{}H", top + row as u16, left);
						stdout.write_all(format!("{}{}", cursor_move, line).as_bytes())?;
					}
				}
			}
		} else {
			for (row, line) in current_frame.iter().enumerate() {
				let cursor_move = format!("\x1B[{};{}H", top + row as u16, left);
				stdout.write_all(format!("{}{}", cursor_move, line).as_bytes())?;
			}
		}

		stdout.flush()?;
		previous_frame = Some(current_frame);

		let elapsed = frame_start.duration_since(start_time);
		frame_index = ((elapsed.as_secs_f64() * OUTPUT_FPS as f64) as usize) % total_frames;

		let frame_end = Instant::now();
		let frame_processing_time = frame_end - frame_start;
		if frame_processing_time < frame_duration {
			sleep(frame_duration - frame_processing_time);
		}
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
	let dimensions: Vec<u32> = output_str.trim().split(',').map(|s| s.parse().unwrap()).collect();

	Ok((dimensions[0], dimensions[1]))
}

fn extract_frames(width: u32, height: u32) -> Result<Vec<DynamicImage>, Box<dyn std::error::Error>> {
	let mut frames = Vec::new();
	let mut child = Command::new("ffmpeg")
		.args(&[
			"-i",
			INPUT,
			"-t",
			&DURATION.to_string(),
			"-r",
			&OUTPUT_FPS.to_string(),
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
		let image_buffer =
			ImageBuffer::from_raw(width, height, buffer.clone()).ok_or("Failed to create image from buffer")?;
		frames.push(DynamicImage::ImageRgb8(image_buffer));
	}

	Ok(frames)
}

fn frame_to_ascii(frame: DynamicImage, target_size: NonZeroU32) -> Vec<String> {
	let config = artem::config::ConfigBuilder::new().target_size(target_size).build();
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
