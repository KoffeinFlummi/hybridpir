use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::{jobjectArray, jbyteArray, jint};

use crate::client::HybridPirClient;

#[no_mangle]
pub unsafe extern fn Java_de_tu_1darmstadt_cs_encrypto_hybridpir_RustInterface_sendQuery(
    env: JNIEnv,
    _: JClass,
    targets: jobjectArray,
    db_size: jint,
    element_size: jint,
    raidpir_redundancy: jint,
    raidpir_size: jint,
    sealpir_degree: jint,
    sealpir_log: jint,
    sealpir_d: jint,
    index: jint
) -> jbyteArray {
    android_log::init("HybridPIR").unwrap();

    let raidpir_servers = env.get_array_length(targets).unwrap() as usize;

    let mut servers: Vec<String> = Vec::with_capacity(raidpir_servers);
    for i in 0..raidpir_servers {
        let object = env.get_object_array_element(targets, i as i32).unwrap();
        let java_str = env.get_string(object.into()).unwrap();
        servers.push(java_str.into());
    }

    let client = HybridPirClient::new(
        db_size as usize,
        element_size as usize,
        raidpir_servers as usize,
        raidpir_redundancy as usize,
        raidpir_size as usize,
        sealpir_degree as u32,
        sealpir_log as u32,
        sealpir_d as u32
    );

    let response = match client.send_query(&servers, index as usize) {
        Ok(r) => r,
        Err(e) => {
            error!("{:?}", e);
            Vec::new()
        }
    };

    env.byte_array_from_slice(&response).unwrap()
}
