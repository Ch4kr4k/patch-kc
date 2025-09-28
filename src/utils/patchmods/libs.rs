use crate::utils::utils;

pub fn diff(configs: &utils::Configs) {
    // check patch_config against kernel_config
    for (key, patch_value) in &configs.patch_config {
        match configs.kernel_config.get(key) {
            Some(kernel_value) => {
                if kernel_value != patch_value {
                    println!(
                        "[-] Difference for key `{}`: kernel=`{}` patch=`{}`",
                        key, kernel_value, patch_value
                    );
                } else {
                    println!(
                        "[+] Matched for key `{}`: kernel=`{}` patch=`{}`",
                        key, kernel_value, patch_value
                    );
                }
            }
            None => {
                println!(
                    "[x] Key `{}` only in patch config with value `{}`",
                    key, patch_value
                );
            }
        }
    }
}
