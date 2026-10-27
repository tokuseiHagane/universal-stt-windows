use crate::DiarizeError;

/// Iterative radix-2 FFT in double, matching the portable Swift path.
pub(crate) struct Dft {
    count: usize,
    reversed: Vec<usize>,
    twiddle_re: Vec<f64>,
    twiddle_im: Vec<f64>,
}

impl Dft {
    pub(crate) fn new(count: usize) -> Result<Self, DiarizeError> {
        if count < 2 || !count.is_power_of_two() {
            return Err(DiarizeError::Model(format!(
                "DFT length {count} is not a power of two"
            )));
        }
        let bits = count.trailing_zeros();
        let reversed = (0..count)
            .map(|i| {
                let mut x = i;
                let mut r = 0usize;
                for _ in 0..bits {
                    r = (r << 1) | (x & 1);
                    x >>= 1;
                }
                r
            })
            .collect();
        let twiddle_re = (0..count / 2)
            .map(|k| (2.0 * std::f64::consts::PI * k as f64 / count as f64).cos())
            .collect();
        let twiddle_im = (0..count / 2)
            .map(|k| -(2.0 * std::f64::consts::PI * k as f64 / count as f64).sin())
            .collect();
        Ok(Self {
            count,
            reversed,
            twiddle_re,
            twiddle_im,
        })
    }

    pub(crate) fn transform(&self, real: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let n = self.count;
        let mut re = vec![0.0f64; n];
        let mut im = vec![0.0f64; n];
        for i in 0..n {
            re[i] = real[self.reversed[i]] as f64;
        }

        let mut size = 2;
        while size <= n {
            let half = size / 2;
            let step = n / size;
            let mut start = 0;
            while start < n {
                let mut k = 0;
                for j in 0..half {
                    let a = start + j;
                    let b = a + half;
                    let wr = self.twiddle_re[k];
                    let wi = self.twiddle_im[k];
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                    k += step;
                }
                start += size;
            }
            size *= 2;
        }

        (
            re.into_iter().map(|v| v as f32).collect(),
            im.into_iter().map(|v| v as f32).collect(),
        )
    }
}
