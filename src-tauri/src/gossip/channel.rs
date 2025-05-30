use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

pub use iroh::NodeId;
use n0_future::{boxed::BoxStream, StreamExt as _};
