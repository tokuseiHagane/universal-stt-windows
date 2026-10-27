use crate::linker::dot;

/// Average-linkage clustering on cosine distance: the closest two clusters merge
/// while their mean pairwise distance is under `1 - cosine`. Labels are numbered
/// in order of first appearance.
pub fn average_linkage(unit: &[Vec<f32>], cosine: f32) -> Vec<usize> {
    let n = unit.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0];
    }
    let threshold = 1.0 - cosine;
    let mut distance = vec![0.0f32; n * n];
    for i in 0..n {
        for j in (i + 1)..n {
            let d = 1.0 - dot(&unit[i], &unit[j]);
            distance[i * n + j] = d;
            distance[j * n + i] = d;
        }
    }
    let mut size = vec![1usize; n];
    let mut alive = vec![true; n];
    let mut member: Vec<usize> = (0..n).collect();

    loop {
        let mut best = f32::INFINITY;
        let mut pair = (usize::MAX, usize::MAX);
        for a in 0..n {
            if !alive[a] {
                continue;
            }
            for b in (a + 1)..n {
                if alive[b] && distance[a * n + b] < best {
                    best = distance[a * n + b];
                    pair = (a, b);
                }
            }
        }
        if pair.0 == usize::MAX || best >= threshold {
            break;
        }
        let (a, b) = pair;
        for c in 0..n {
            if alive[c] && c != a && c != b {
                let merged = (size[a] as f32 * distance[a * n + c]
                    + size[b] as f32 * distance[b * n + c])
                    / (size[a] + size[b]) as f32;
                distance[a * n + c] = merged;
                distance[c * n + a] = merged;
            }
        }
        size[a] += size[b];
        alive[b] = false;
        for slot in &mut member {
            if *slot == b {
                *slot = a;
            }
        }
    }

    let mut labels = vec![0usize; n];
    let mut numbered = std::collections::HashMap::new();
    let mut next = 0;
    for i in 0..n {
        let label = *numbered.entry(member[i]).or_insert_with(|| {
            let value = next;
            next += 1;
            value
        });
        labels[i] = label;
    }
    labels
}
