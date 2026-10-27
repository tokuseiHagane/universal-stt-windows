use crate::DiarizeError;

pub fn parse_banks(bytes: &[u8]) -> Result<(Vec<f32>, usize, usize), DiarizeError> {
    if bytes.len() < 8 {
        return Err(DiarizeError::Model("mel bank table is truncated".into()));
    }
    let mels = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let bins = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    if mels == 0 || bins == 0 || bytes.len() != 8 + 4 * mels * bins {
        return Err(DiarizeError::Model(format!(
            "mel bank table of {mels} x {bins} does not match {} bytes",
            bytes.len()
        )));
    }
    let mut banks = Vec::with_capacity(mels * bins);
    for start in (8..bytes.len()).step_by(4) {
        banks.push(f32::from_le_bytes(
            bytes[start..start + 4].try_into().unwrap(),
        ));
    }
    Ok((banks, mels, bins))
}
