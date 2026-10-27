#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WavePoint {
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WaveData {
    pub bucket_ms: u32,
    pub peaks: Vec<WavePoint>,
}
