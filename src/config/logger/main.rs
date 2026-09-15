use super::{tracing::TracingEngine};



enum LoggerType {
    TRACING(TracingEngine),
}
pub trait LoggerProvider {
    fn init(&self);
}



fn init(logger: LoggerType) {
    match logger {
        LoggerType::TRACING(tracing_engine) => tracing_engine.init(),
    }
}

pub fn initialize(){
    let _tracing = LoggerType::TRACING(TracingEngine);

    init(_tracing);
}