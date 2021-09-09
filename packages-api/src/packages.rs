use fire_crypto::signature::PublicKey;


pub struct Source {
    /// example packages.lvgd.ch:9281
    url: String,
    /// if public == false an authentication token is sent?
    public: bool,
    /// the signature key
    public_key: PublicKey

    // todo add whitelist that only specific packages can be fetched
    // from this source
}

pub struct PackagesCfg {
    list: Vec<String>,
    /// sources to fetch for updates
    /// updates are checked in reverse order
    /// until some source is found that contains that package
    sources: Vec<Source>,
    /// if this is true that last source will return realtime updates
    fetch_realtime: bool,
    /// the package that should be run when installing
    on_install: String,
    /// the package that should be run normally
    on_run: String
}


pub struct Package {
    name: String,
    version_str: String,
    /// blake2s hash of the compressed file
    version: String,
    current: String,
    binary: Option<String>
}