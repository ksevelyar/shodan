use std::{io, path::Path};

use tokio::io::{AsyncBufReadExt, BufReader};

pub(super) fn describe() -> serde_json::Value {
    serde_json::json!({"type":"function","function":{"name":"read","description":"Read a text file or image","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path"},"offset":{"type":"integer","description":"First line, starting at 1 for text files"},"limit":{"type":"integer","description":"Maximum number of lines for text files"}},"required":["path"]}}})
}

pub(super) async fn read_image(path: &Path) -> io::Result<Option<(String, String)>> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path).await?;
    let mut header = [0; 32];
    let count = file.read(&mut header).await?;
    let Ok(
        format @ (image::ImageFormat::Png
        | image::ImageFormat::Jpeg
        | image::ImageFormat::Gif
        | image::ImageFormat::WebP
        | image::ImageFormat::Bmp),
    ) = image::guess_format(&header[..count])
    else {
        return Ok(None);
    };
    let mut bytes = header[..count].to_vec();
    file.read_to_end(&mut bytes).await?;
    tokio::task::spawn_blocking(move || process_image(bytes, format))
        .await
        .map_err(io::Error::other)?
        .map(Some)
}

fn process_image(bytes: Vec<u8>, format: image::ImageFormat) -> io::Result<(String, String)> {
    use base64::Engine;
    use image::GenericImageView;

    let media_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        image::ImageFormat::Bmp => "image/bmp",
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported image format",
            ));
        }
    };
    let image = image::load_from_memory_with_format(&bytes, format).map_err(io::Error::other)?;
    let (width, height) = image.dimensions();
    let converted = width > 2000
        || height > 2000
        || bytes.len() > 2_000_000
        || format == image::ImageFormat::Bmp;
    let (bytes, media_type, description) = if converted {
        let image = if image.color().has_alpha() {
            let mut background =
                image::RgbaImage::from_pixel(width, height, image::Rgba([255, 255, 255, 255]));
            image::imageops::overlay(&mut background, &image.to_rgba8(), 0, 0);
            image::DynamicImage::ImageRgba8(background)
        } else {
            image
        };
        let image = if width > 2000 || height > 2000 {
            image.resize(2000, 2000, image::imageops::FilterType::Lanczos3)
        } else {
            image
        };
        let (output_width, output_height) = image.dimensions();
        let mut output = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 90)
            .encode_image(&image.to_rgb8())
            .map_err(io::Error::other)?;
        (
            output,
            "image/jpeg",
            format!(
                "Read image file [image/jpeg] original {width}x{height} output {output_width}x{output_height}"
            ),
        )
    } else {
        (bytes, media_type, format!("Read image file [{media_type}]"))
    };
    Ok((
        description,
        format!(
            "data:{media_type};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ),
    ))
}

pub(super) async fn read_file(
    path: &Path,
    offset: Option<usize>,
    limit: Option<usize>,
) -> io::Result<String> {
    read_file_with_context(path, offset, limit)
        .await
        .map(|(content, _)| content)
}

pub(super) async fn read_file_with_context(
    path: &Path,
    offset: Option<usize>,
    limit: Option<usize>,
) -> io::Result<(String, String)> {
    let first_line = offset.unwrap_or(1);
    if first_line == 0 || limit == Some(0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "offset and limit must be greater than zero",
        ));
    }
    let file = tokio::fs::File::open(path).await?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut preceding_content = String::new();
    for _ in 1..first_line {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "offset is beyond the end of the file",
            ));
        }
        preceding_content.push_str(&line);
    }
    let mut content = String::new();
    for line_number in 0..limit.unwrap_or(usize::MAX) {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            if line_number == 0 && first_line > 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "offset is beyond the end of the file",
                ));
            }
            break;
        }
        content.push_str(&line);
    }
    Ok((content, preceding_content))
}
