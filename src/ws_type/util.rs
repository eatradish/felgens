use anyhow::anyhow;

pub fn trans_err(name: &str, step: u32) -> anyhow::Error {
    anyhow!("Can not get {} step {}", name, step)
}
