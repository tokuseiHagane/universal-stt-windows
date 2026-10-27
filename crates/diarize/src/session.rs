use crate::DiarizeError;
use crate::meta::Meta;
use ort::session::Session;
use ort::value::Tensor as OrtTensor;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    pub data: Vec<f32>,
    pub shape: Vec<usize>,
}

impl Tensor {
    pub fn new(data: Vec<f32>, shape: Vec<usize>) -> Self {
        Self { data, shape }
    }
}

pub struct Sessions {
    _env: ort::environment::Environment,
    sessions: HashMap<String, Session>,
}

impl Sessions {
    pub fn load(dir: impl AsRef<Path>, meta: &Meta) -> Result<Self, DiarizeError> {
        let dir = dir.as_ref();
        let env = ort::init()
            .build()
            .map_err(|e| DiarizeError::Ort(e.to_string()))?;
        let mut names = Vec::new();
        for window in &meta.windows {
            names.push(window.encoder.compiled.clone());
            names.push(window.queries.compiled.clone());
        }
        names.push(meta.voice.compiled.clone());
        names.sort();
        names.dedup();

        let mut sessions = HashMap::new();
        for name in names {
            let path = dir.join(&name);
            let session = Session::builder(&env)
                .map_err(|e| DiarizeError::Ort(e.to_string()))?
                .commit_from_file(&path)
                .map_err(|e| DiarizeError::Ort(format!("{}: {e}", path.display())))?;
            sessions.insert(name, session);
        }
        Ok(Self {
            _env: env,
            sessions,
        })
    }

    pub fn run(
        &mut self,
        graph: &str,
        inputs: Vec<(&str, Tensor)>,
        outputs: &[&str],
    ) -> Result<Vec<Tensor>, DiarizeError> {
        let session = self
            .sessions
            .get_mut(graph)
            .ok_or_else(|| DiarizeError::Model(format!("session {graph} is not loaded")))?;

        let mut values = Vec::with_capacity(inputs.len());
        for (name, tensor) in inputs {
            let value = OrtTensor::from_array((tensor.shape, tensor.data))
                .map_err(|e| DiarizeError::Ort(e.to_string()))?;
            values.push((name, value));
        }

        let results = session
            .run(values)
            .map_err(|e| DiarizeError::Ort(format!("{graph}: {e}")))?;

        let mut out = Vec::with_capacity(outputs.len());
        for name in outputs {
            let value = results
                .get(*name)
                .ok_or_else(|| DiarizeError::Model(format!("{graph} has no output {name}")))?;
            let array = value
                .try_extract_array::<f32>()
                .map_err(|e| DiarizeError::Ort(format!("{graph}.{name}: {e}")))?;
            out.push(Tensor {
                data: array.iter().copied().collect(),
                shape: array.shape().to_vec(),
            });
        }
        Ok(out)
    }
}
