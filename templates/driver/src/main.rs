use librootware::{
    capability::{capability_kind, Capability, CapabilityProvider, InMemoryCapabilities},
    info,
};

fn main() {
    let mut capabilities = InMemoryCapabilities::default();
    capabilities
        .request(Capability { id: capability_kind::IPC_SEND })
        .expect("request driver capability");
    info!("{{project_name}} RKM driver initialized");
}
