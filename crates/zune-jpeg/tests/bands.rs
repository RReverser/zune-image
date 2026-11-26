/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for `decode_into_with` and `decode_stripes_with`, which hand over bands of rows during
//! the decoding.

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

/// Decodes `data` with `decode_into_with` and checks that the bands cover every row once, in
/// order, and hold the pixels that `decode` returns. Returns the number of bands.
fn assert_bands_match_decode(data: &[u8], colorspace: ColorSpace) -> usize {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    let expected = JpegDecoder::new_with_options(ZCursor::new(data), options)
        .decode()
        .unwrap();

    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    let info = decoder.info().unwrap();
    let row_bytes = usize::from(info.width) * colorspace.num_components();
    let mut out = vec![0; decoder.output_buffer_size().unwrap()];
    let mut bands = vec![];
    decoder
        .decode_into_with(&mut out, |rows, band| bands.push((rows, band.to_vec())))
        .unwrap();

    let mut next_row = 0;
    for (rows, band) in &bands {
        assert_eq!(
            rows.start, next_row,
            "the bands are in order and leave no gap"
        );
        assert!(!rows.is_empty(), "a band holds rows");
        assert_eq!(band.len(), rows.len() * row_bytes, "a band holds its rows");
        assert_eq!(
            band[..],
            expected[rows.start * row_bytes..rows.end * row_bytes],
            "rows {rows:?}"
        );
        next_row = rows.end;
    }
    assert_eq!(
        next_row,
        usize::from(info.height),
        "the bands cover the image"
    );
    assert_eq!(out, expected);
    assert_stripes_match_decode(data, colorspace, &expected);
    bands.len()
}

/// Decodes `data` with `decode_stripes_with` and checks that the stripes cover every row once,
/// in order, and hold the pixels `expected` that `decode` returns. Every other stripe takes the
/// buffer, so that the decoder writes both into the buffer it handed over and into a new one.
fn assert_stripes_match_decode(data: &[u8], colorspace: ColorSpace, expected: &[u8]) {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    let info = decoder.info().unwrap();
    let row_bytes = usize::from(info.width) * colorspace.num_components();
    let mut stripes = vec![];
    decoder
        .decode_stripes_with(|rows, stripe| {
            if stripes.len() % 2 == 0 {
                stripes.push((rows, std::mem::take(stripe)));
            } else {
                stripes.push((rows, stripe.clone()));
            }
        })
        .unwrap();

    let mut next_row = 0;
    for (rows, stripe) in &stripes {
        assert_eq!(
            rows.start, next_row,
            "the stripes are in order and leave no gap"
        );
        assert!(!rows.is_empty(), "a stripe holds rows");
        assert!(rows.len() <= 36, "a stripe holds a row of MCUs at most");
        assert_eq!(
            stripe.len(),
            rows.len() * row_bytes,
            "a stripe holds its rows"
        );
        assert_eq!(
            stripe[..],
            expected[rows.start * row_bytes..rows.end * row_bytes],
            "rows {rows:?}"
        );
        next_row = rows.end;
    }
    assert_eq!(
        next_row,
        usize::from(info.height),
        "the stripes cover the image"
    );
}

#[test]
fn bands_of_sequential_images_match_decode() {
    for (data, colorspace) in [
        (
            &include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg")[..],
            ColorSpace::RGB
        ),
        (
            &include_bytes!("../../../test-images/jpeg/medium_horiz_samp_2500x1786.jpg")[..],
            ColorSpace::RGB
        ),
        (
            &include_bytes!("../../../test-images/jpeg/medium_vertical_samp_2500x1786.jpg")[..],
            ColorSpace::RGB
        ),
        (
            &include_bytes!("../../../test-images/jpeg/medium_vertical_samp_2500x1786.jpg")[..],
            ColorSpace::Luma
        ),
        (
            &include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg")[..],
            ColorSpace::Luma
        )
    ] {
        // A sequential image hands over a band for each row of MCUs or two.
        assert!(assert_bands_match_decode(data, colorspace) > 1);
    }
}

#[test]
fn bands_of_images_complete_after_the_last_scan_match_decode() {
    for data in [
        &include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg")[..],
        &include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg")[..],
        &include_bytes!("../../../test-images/jpeg/sos_news.jpeg")[..]
    ] {
        assert_bands_match_decode(data, ColorSpace::RGB);
    }
}

#[test]
fn bands_of_an_incomplete_image_match_decode() {
    assert_bands_match_decode(
        include_bytes!("../../../test-images/jpeg/incomplete_image.jpg"),
        ColorSpace::RGB
    );
}
