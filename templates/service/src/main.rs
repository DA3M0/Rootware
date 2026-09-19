use librootware::{
    capability::Capability,
    ipc::{message_type, Message, PAYLOAD_SIZE},
    service::{EchoService, ServiceRegistry},
    info,
};

fn main() -> librootware::Result<()> {
    let mut services = ServiceRegistry::new();
    services.register(Box::new(EchoService::new()))?;
    services.start()?;

    let request = Message::new(
        1,
        2,
        message_type::REQUEST,
        Capability { id: 1 },
        [0; PAYLOAD_SIZE],
    );
    let _response = services.dispatch("echo", &request)?;
    info!("{{project_name}} service started");
    services.stop();
    Ok(())
}
