use super::*;

#[test]
fn a_round_trip_returns_the_same_samples_and_rate() {
    // Powers of two, so every one is exact on both sides: the file holds
    // 32-bit integers and the core holds f32, and a sample has to
    // survive the trip through both.
    let samples = [0.0, 0.5, -0.5, -1.0, 0.125, -0.0625];
    let (read, rate) = decode(&encode(&samples, 44_100)).expect("decodes");

    assert_eq!(read, samples);
    assert_eq!(rate, 44_100);
}

/// The core moved to f32; the file deliberately did not. Loops recorded
/// by an earlier build still have to load, and the saved loop still has
/// to be something an audio editor can open.
#[test]
fn the_file_still_holds_32_bit_integers() {
    let wav = encode(&[0.5, -1.0], 44_100);
    let data = &wav[44..];
    assert_eq!(wav.len(), 44 + 2 * 4);
    assert_eq!(i32::from_le_bytes(data[0..4].try_into().unwrap()), 1 << 30);
    assert_eq!(i32::from_le_bytes(data[4..8].try_into().unwrap()), i32::MIN);
}

#[test]
fn an_empty_loop_round_trips_too() {
    let (read, rate) = decode(&encode(&[], 48_000)).expect("decodes");
    assert!(read.is_empty());
    assert_eq!(rate, 48_000);
}

#[test]
fn the_header_is_the_expected_44_bytes() {
    let wav = encode(&[0.5, 0.25, 0.125], 44_100);
    assert_eq!(wav.len(), 44 + 3 * 4);
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    // RIFF size counts everything after its own size field.
    assert_eq!(
        u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize,
        wav.len() - 8
    );
}

#[test]
fn chunks_we_do_not_write_are_skipped() {
    // A "LIST" chunk between fmt and data, as plenty of editors add.
    let mut wav = encode(&[0.25, -0.25], 44_100);
    let list = {
        let mut chunk = b"LIST".to_vec();
        chunk.extend_from_slice(&4u32.to_le_bytes());
        chunk.extend_from_slice(b"INFO");
        chunk
    };
    wav.splice(36..36, list.iter().copied());
    let riff_size = (wav.len() - 8) as u32;
    wav[4..8].copy_from_slice(&riff_size.to_le_bytes());

    let (read, rate) = decode(&wav).expect("decodes past the extra chunk");
    assert_eq!(read, [0.25, -0.25]);
    assert_eq!(rate, 44_100);
}

#[test]
fn anything_that_is_not_a_wav_is_rejected() {
    assert!(decode(b"").is_none());
    assert!(decode(b"not a wav at all").is_none());
    assert!(decode(&encode(&[0.5], 44_100)[..20]).is_none(), "truncated");
}

#[test]
fn a_format_we_cannot_use_is_rejected() {
    let stereo = {
        let mut wav = encode(&[0.5, 0.25], 44_100);
        wav[22..24].copy_from_slice(&2u16.to_le_bytes());
        wav
    };
    assert!(decode(&stereo).is_none(), "two channels");

    let sixteen_bit = {
        let mut wav = encode(&[0.5, 0.25], 44_100);
        wav[34..36].copy_from_slice(&16u16.to_le_bytes());
        wav
    };
    assert!(decode(&sixteen_bit).is_none(), "16-bit");
}
