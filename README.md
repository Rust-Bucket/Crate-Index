
[![Released API docs](https://docs.rs/crate-index/badge.svg)](https://docs.rs/crate-index)
![Continuous integration](https://github.com/Rust-Bucket/cargo-registry/workflows/Continuous%20integration/badge.svg)
[![Coverage Status](https://codecov.io/gh/Rust-Bucket/Crate-Index/branch/master/graph/badge.svg)](https://codecov.io/gh/Rust-Bucket/Crate-Index)

# Crate Index

Crate-Index is a library for managing and manipulating a Cargo crate
registry.

*see the [cargo docs](https://doc.rust-lang.org/cargo/reference/registries.html#running-a-registry) for details*

## Basic Usage
```rust
use crate_index::{Index, Url, Metadata, Version};

// Create a new index, backed by the filesystem and a git repository
let root = "/index";
let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

let mut index = Index::initialise(root, download)
    .build()
    .await?;

// Create a new crate 'Metadata' object
let name = "foo";
let version = Version::parse("0.1.0").unwrap();
let check_sum = "d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c";

let metadata = Metadata::new(name, version, check_sum);

// Insert the Metadata into the index
index.insert(metadata).await?;

```

## Requirements

Crate-Index requires a minimum compiler version of 1.39.0

## License

* MIT license (https://opensource.org/licenses/MIT)
