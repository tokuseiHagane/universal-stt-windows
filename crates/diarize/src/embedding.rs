use crate::DiarizeError;
use crate::linker::unit;

#[derive(Debug, Clone)]
pub struct Embedding {
    /// Row-major `[out * in]`.
    pub weight: Vec<f32>,
    pub bias: Vec<f32>,
    pub out: usize,
    pub columns: usize,
}

impl Embedding {
    /// `[out, in]` as two little-endian u32, then the weight rows, then the bias, as little-endian f16.
    pub fn from_sidecar(bytes: &[u8]) -> Result<Self, DiarizeError> {
        if bytes.len() < 8 {
            return Err(DiarizeError::Model("embedding sidecar is truncated".into()));
        }
        let out = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let inner = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        if out == 0
            || inner == 0
            || !inner.is_multiple_of(2)
            || bytes.len() != 8 + 2 * (out * inner + out)
        {
            return Err(DiarizeError::Model(format!(
                "embedding sidecar of {out} x {inner} does not match {} bytes",
                bytes.len()
            )));
        }
        let mut values = Vec::with_capacity(out * inner + out);
        for start in (8..bytes.len()).step_by(2) {
            values.push(half(u16::from_le_bytes(
                bytes[start..start + 2].try_into().unwrap(),
            )));
        }
        Ok(Self {
            weight: values[..out * inner].to_vec(),
            bias: values[out * inner..].to_vec(),
            out,
            columns: inner / 2,
        })
    }

    pub fn pool(&self) -> Pool {
        Pool::new(self.columns)
    }

    /// Unit embedding of a pool, or `None` for fewer than two columns.
    pub fn embed(&self, pool: &Pool) -> Option<Vec<f32>> {
        if pool.count < 2 {
            return None;
        }
        let n = pool.count as f64;
        let mut stats = vec![0.0f32; 2 * self.columns];
        for c in 0..self.columns {
            let mean = pool.sum[c] / n;
            let variance = ((pool.squares[c] - n * mean * mean) / (n - 1.0)).max(0.0);
            stats[c] = mean as f32;
            stats[self.columns + c] = variance.sqrt() as f32;
        }
        let mut result = self.bias.clone();
        for (o, bias) in result.iter_mut().enumerate().take(self.out) {
            let row = o * 2 * self.columns;
            let mut acc = 0.0f32;
            for (i, &stat) in stats.iter().enumerate() {
                acc += self.weight[row + i] * stat;
            }
            *bias += acc;
        }
        Some(unit(&result))
    }
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub sum: Vec<f64>,
    pub squares: Vec<f64>,
    pub count: usize,
}

impl Pool {
    pub fn new(columns: usize) -> Self {
        Self {
            sum: vec![0.0; columns],
            squares: vec![0.0; columns],
            count: 0,
        }
    }

    pub fn add(&mut self, values: &[f32], offset: usize) {
        for c in 0..self.sum.len() {
            let x = values[offset + c] as f64;
            self.sum[c] += x;
            self.squares[c] += x * x;
        }
        self.count += 1;
    }
}

fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = ((bits >> 10) & 0x1F) as i32;
    let fraction = (bits & 0x03FF) as f32;
    match exponent {
        0 => sign * fraction * 2f32.powi(-24),
        0x1F => {
            if fraction == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => sign * (1.0 + fraction / 1024.0) * 2f32.powi(exponent - 15),
    }
}
