use super::*;

#[test]
fn a_round_trip_returns_the_same_samples_and_rate() {
    let samples = [0, 1, -1, i32::MAX, i32::MIN, 12_345];
    let (read, rate) = decode(&encode(&samples, 44_100)).expect("decodes");

    assert_eq!(read, samples);
    assert_eq!(rate, 44_100);
}

#[test]
fn an_empty_loop_round_trips_too() {
    let (read, rate) = decode(&encode(&[], 48_000)).expect("decodes");
    assert!(read.is_empty());
    assert_eq!(rate, 48_000);
}

#[test]
fn the_header_is_the_expected_44_bytes() {
    let wav = encode(&[1, 2, 3], 44_100);
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
    let mut wav = encode(&[7, 8], 44_100);
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
    assert_eq!(read, [7, 8]);
    assert_eq!(rate, 44_100);
}

#[test]
fn anything_that_is_not_a_wav_is_rejected() {
    assert!(decode(b"").is_none());
    assert!(decode(b"not a wav at all").is_none());
    assert!(decode(&encode(&[1], 44_100)[..20]).is_none(), "truncated");
}

#[test]
fn a_format_we_cannot_use_is_rejected() {
    let stereo = {
        let mut wav = encode(&[1, 2], 44_100);
        wav[22..24].copy_from_slice(&2u16.to_le_bytes());
        wav
    };
    assert!(decode(&stereo).is_none(), "two channels");

    let sixteen_bit = {
        let mut wav = encode(&[1, 2], 44_100);
        wav[34..36].copy_from_slice(&16u16.to_le_bytes());
        wav
    };
    assert!(decode(&sixteen_bit).is_none(), "16-bit");
}
