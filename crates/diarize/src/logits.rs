use crate::DiarizeError;
use crate::Frontend;
use crate::meta::Meta;
use crate::session::{Sessions, Tensor};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub struct WhoLogits {
    pub presence: Vec<f32>,
    pub masks: Vec<f32>,
    pub queries: Vec<f32>,
}

/// One piece through both graphs. The piece's log-mel is computed here against
/// the recording's means; the last frame is repeated to fill the window.
pub fn compute_logits(
    sessions: &mut Sessions,
    meta: &Meta,
    frontend: &Frontend,
    samples: &[f32],
    piece: Range<usize>,
    means: &[f32],
) -> Result<WhoLogits, DiarizeError> {
    if piece.is_empty() {
        return Err(DiarizeError::Model("empty piece".into()));
    }
    let mels = meta.frontend.n_mels;
    let dim = meta.model.dim;
    let queries = meta.model.queries;
    let window = meta.window_for(piece.len());

    let mel = frontend.features(samples, piece.clone(), means);
    let mut input = vec![0.0f32; window.frames * mels];
    input[..mel.len()].copy_from_slice(&mel);
    let last = &mel[mel.len() - mels..];
    for frame in piece.len()..window.frames {
        input[frame * mels..(frame + 1) * mels].copy_from_slice(last);
    }

    let encoded = sessions.run(
        &window.encoder.compiled,
        vec![(
            window.encoder.inputs[0].as_str(),
            Tensor::new(input, vec![1, window.frames, mels]),
        )],
        &[
            window.encoder.outputs[0].as_str(),
            window.encoder.outputs[1].as_str(),
        ],
    )?;
    let memory = &encoded[0].data;
    let frames_out = &encoded[1].data;
    if memory.len() != window.low * dim || frames_out.len() != window.frames * dim {
        return Err(DiarizeError::Model(format!(
            "encoder shapes: memory {} (want {}), frames {} (want {})",
            memory.len(),
            window.low * dim,
            frames_out.len(),
            window.frames * dim
        )));
    }

    let mut inputs: Vec<(&str, Tensor)> = vec![(
        window.queries.inputs[0].as_str(),
        Tensor::new(memory.clone(), vec![1, window.low, dim]),
    )];
    let mut at = 0;
    for (index, chunk) in window.chunks.iter().enumerate() {
        let slice = frames_out[at * dim..(at + chunk) * dim].to_vec();
        inputs.push((
            window.queries.inputs[index + 1].as_str(),
            Tensor::new(slice, vec![1, *chunk, dim]),
        ));
        at += chunk;
    }
    let output_names: Vec<&str> = window.queries.outputs.iter().map(String::as_str).collect();
    let out = sessions.run(&window.queries.compiled, inputs, &output_names)?;

    if out[0].data.len() != queries || out[1].data.len() != queries * dim {
        return Err(DiarizeError::Model(
            "queries graph returned unexpected presence/queries sizes".into(),
        ));
    }
    let mut mask_chunks = Vec::with_capacity(out.len() - 2);
    for (index, chunk) in window.chunks.iter().enumerate() {
        let tensor = &out[index + 2];
        if tensor.data.len() != queries * chunk {
            return Err(DiarizeError::Model(format!(
                "masks_{index} has {} values, want {}",
                tensor.data.len(),
                queries * chunk
            )));
        }
        mask_chunks.push(tensor.data.clone());
    }

    Ok(WhoLogits {
        presence: out[0].data.clone(),
        queries: out[1].data.clone(),
        masks: concat_mask_chunks(&mask_chunks, &window.chunks, piece.len(), queries),
    })
}

/// The chunks' mask logits as one `[queries * frames]`, the padding dropped.
pub fn concat_mask_chunks(
    outputs: &[Vec<f32>],
    chunks: &[usize],
    frames: usize,
    queries: usize,
) -> Vec<f32> {
    let mut out = vec![0.0f32; queries * frames];
    let mut at = 0;
    for (output, chunk) in outputs.iter().zip(chunks) {
        let keep = (*chunk).min(frames.saturating_sub(at));
        if keep == 0 {
            break;
        }
        for query in 0..queries {
            let src = query * *chunk;
            let dst = query * frames + at;
            out[dst..dst + keep].copy_from_slice(&output[src..src + keep]);
        }
        at += chunk;
    }
    out
}
