use super::types::*;
use crate::utils::escape::*;
use anyhow::Result;

pub struct TextGenerator {
    data: String,
}

impl TextGenerator {
    pub fn new() -> Self {
        TextGenerator {
            data: String::new(),
        }
    }

    pub fn generate(mut self, v: &Value) -> Result<String> {
        for (i, item) in v.members().enumerate() {
            match item {
                Value::Str(s) => {
                    self.data.push_str(s);
                }
                Value::Float(_) => {
                    return Err(anyhow::anyhow!(
                        "Unexpected float value at {} in text: item={:?}, {:?}",
                        i,
                        item,
                        v
                    ));
                }
                Value::Int(_) => {
                    return Err(anyhow::anyhow!(
                        "Unexpected int value at {} in text: item={:?}, {:?}",
                        i,
                        item,
                        v
                    ));
                }
                Value::KeyVal((k, _)) => {
                    if k != "name" {
                        return Err(anyhow::anyhow!(
                            "Unexpected key at {} in text: item={:?}, {:?}",
                            i,
                            item,
                            v
                        ));
                    }
                }
                Value::Array(arr) => {
                    self.data.push('<');
                    let mut first = true;
                    for item in arr {
                        if !first {
                            self.data.push(' ');
                        }
                        first = false;
                        match item {
                            Value::Str(s) => {
                                self.data.push_str(s);
                            }
                            Value::Float(f) => {
                                if f.fract() == 0.0 {
                                    self.data.push_str(&format!("{:.1}", f));
                                } else {
                                    self.data.push_str(&f.to_string());
                                }
                            }
                            Value::Int(i) => {
                                self.data.push_str(&i.to_string());
                            }
                            Value::KeyVal((k, v)) => {
                                self.data.push_str(k);
                                self.data.push('=');
                                match v.as_ref() {
                                    Value::Str(s) => {
                                        self.data.push('"');
                                        self.data.push_str(&escape_xml_attr_value(s));
                                        self.data.push('"');
                                    }
                                    Value::Float(f) => {
                                        if f.fract() == 0.0 {
                                            self.data.push_str(&format!("{:.1}", f));
                                        } else {
                                            self.data.push_str(&f.to_string());
                                        }
                                    }
                                    Value::Int(i) => {
                                        self.data.push_str(&i.to_string());
                                    }
                                    Value::Null => {}
                                    _ => {
                                        return Err(anyhow::anyhow!(
                                            "Unexpected value type in text: item={:?}, {:?}",
                                            item,
                                            arr
                                        ));
                                    }
                                }
                            }
                            Value::Array(_) => {
                                return Err(anyhow::anyhow!(
                                    "Unexpected nested array in text: item={:?}, {:?}",
                                    item,
                                    arr
                                ));
                            }
                            _ => {
                                first = true;
                            }
                        }
                    }
                    self.data.push('>');
                }
                _ => {}
            }
        }
        Ok(self.data)
    }
}
