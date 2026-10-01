use cpal::traits::{DeviceTrait, HostTrait};
fn main() {
    let host = cpal::default_host();
    let d = host.default_input_device().unwrap();
    println!("device: {}", d.name().unwrap());
    let c = d.default_input_config().unwrap();
    println!("default config: {:?} {}Hz {}ch buffer={:?}", c.sample_format(), c.sample_rate().0, c.channels(), c.buffer_size());
    for c in d.supported_input_configs().unwrap() {
        println!("  supported: {:?} min={}Hz max={}Hz ch={} buf={:?}", c.sample_format(), c.min_sample_rate().0, c.max_sample_rate().0, c.channels(), c.buffer_size());
    }
}
