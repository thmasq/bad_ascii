use artem::convert;
use image::{DynamicImage, ImageBuffer};
use proc_macro::TokenStream;
use quote::quote;
use std::num::NonZeroU32;
use std::process::Command;
use syn::{LitStr, parse_macro_input};

const OUTPUT_FPS: u64 = 24;
const DURATION: u64 = 10;

#[proc_macro]
pub fn process(input: TokenStream) -> TokenStream {
	let input_path = parse_macro_input!(input as LitStr).value();
	let frames = extract_frames(&input_path).expect("Failed to extract frames");
	let ascii_frames: Vec<String> = frames.into_iter().map(|frame| frame_to_ascii(frame)).collect();

	let frame_count = ascii_frames.len();
	let total_chars: usize = ascii_frames.iter().map(|s| s.len()).sum();

	let frame_lengths: Vec<usize> = ascii_frames.iter().map(|s| s.len()).collect();
	let frame_length_array = frame_lengths.iter().map(|&len| quote! { #len });

	let all_chars: String = ascii_frames.join("");
	let char_array = all_chars.chars().map(|c| quote! { #c });

	let expanded = quote! {
		#[allow(clippy::all)]
		mod ascii_frames {
			use std::mem::MaybeUninit;

			const FRAME_COUNT: usize = #frame_count;
			const TOTAL_CHARS: usize = #total_chars;

			const FRAME_LENGTHS: [usize; FRAME_COUNT] = [#(#frame_length_array),*];
			const CHAR_ARRAY: [char; TOTAL_CHARS] = [#(#char_array),*];

			const fn create_frames() -> [&'static str; FRAME_COUNT] {
				let mut frames: [&str; FRAME_COUNT] = [""; FRAME_COUNT];
				let mut char_index = 0;
				let mut i = 0;
				while i < FRAME_COUNT {
					let length = FRAME_LENGTHS[i];
					// SAFETY: We ensure that char_index and length are within bounds
					frames[i] = unsafe {
						std::str::from_utf8_unchecked(
							std::slice::from_raw_parts(
								CHAR_ARRAY.as_ptr().add(char_index) as *const u8,
								length
							)
						)
					};
					char_index += length;
					i += 1;
				}
				frames
			}

			pub static ASCII_FRAMES: [&'static str; FRAME_COUNT] = create_frames();
		}

		use self::ascii_frames::ASCII_FRAMES;
	};

	expanded.into()
}

fn extract_frames(input: &str) -> Result<Vec<DynamicImage>, Box<dyn std::error::Error>> {
	let (width, height) = get_video_dimensions(input)?;
	let mut frames = Vec::new();
	let output = Command::new("ffmpeg")
		.args(&[
			"-i",
			input,
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
		.output()?;

	let buffer = output.stdout;
	let chunk_size = (width * height * 3) as usize;

	for chunk in buffer.chunks(chunk_size) {
		if chunk.len() == chunk_size {
			let image_buffer =
				ImageBuffer::from_raw(width, height, chunk.to_vec()).ok_or("Failed to create image from buffer")?;
			frames.push(DynamicImage::ImageRgb8(image_buffer));
		}
	}

	Ok(frames)
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

fn frame_to_ascii(frame: DynamicImage) -> String {
	let config = artem::config::ConfigBuilder::new()
		.target_size(NonZeroU32::new(160).unwrap())
		.build();
	convert(frame, &config)
}
