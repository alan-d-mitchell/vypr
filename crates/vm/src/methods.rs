use std::{cell::RefCell, collections::HashMap, rc::Rc};
use error::error::VyprError;
use crate::{value::{DataType, DictKey, SharedFile, Value::{self}}, vm::{MethodCache, VM}};

impl VM {

    pub(crate) fn invoke_method(&mut self, name_idx: usize, arg_count: usize, current_ip: usize) -> Result<(), VyprError> {
        let method_name = self.read_string(name_idx)?;

        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.pop()?);
        }
        args.reverse();

        let obj = self.pop()?;

        let cache_type = if let Some(s) = obj.as_str() {
            self.invoke_string_method(s, &method_name, &args)?
        } else {
            match obj {
                Value::List(items) => self.invoke_list_method(items, &method_name, &args)?,
                Value::Dict(dict) => self.invoke_dict_method(dict, &method_name, &args)?,
                Value::File(file) => self.invoke_file_method(file, &method_name, &args)?,
                val => return Err(self.error("R004", format!("object {:?} has no method '{}'", val.get_type(), method_name))),
            }
        };

        if cache_type != MethodCache::EMPTY {
            self.set_cache(current_ip, cache_type);
        }

        Ok(())
    }

    fn invoke_list_method(&mut self, items: Rc<RefCell<Vec<Value>>>, method: &str, args: &[Value]) -> Result<MethodCache, VyprError> {
        match method {
            "append" => {
                if args.len() > 1 {
                    return Err(self.error("R006", "append() takes exactly 1 argument").with_help("remove the extra arguments"));
                }
                if args.is_empty() {
                    return Err(self.error("R006", "append() takes exactly 1 argument").with_help("add an argument"));
                }

                items.borrow_mut().push(args[0].clone());
                self.push(Value::None);

                Ok(MethodCache::ListAppend)
            }

            "clear" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "clear() takes no arguments").with_help("remove the arguments"));
                }

                items.borrow_mut().clear();
                self.push(Value::None);

                Ok(MethodCache::EMPTY)
            }

            "insert" => {
                if args.len() != 2 {
                    let hint = if args.len() > 2 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("R006", format!("insert() takes exactly 2 arguments, got {}", args.len())).with_help(hint));
                }

                let index = match args[0] {
                    Value::Int(i) => i,
                    _ => return Err(self.error("R002", "insert() index must be an integer")),
                };

                let value = args[1].clone();
                let mut borrowed_items = items.borrow_mut();
                let len = borrowed_items.len() as i64;

                if index >= len {
                    borrowed_items.push(value);
                } else if index < 0 {
                    let effective_index = len + index;

                    if effective_index < 0 {
                        borrowed_items.insert(0, value);
                    } else {
                        borrowed_items.insert(effective_index as usize, value);
                    }
                } else {
                    borrowed_items.insert(index as usize, value);
                }

                self.push(Value::None);
                Ok(MethodCache::EMPTY)
            }

            "pop" => {
                if args.len() > 1 {
                    return Err(self.error("R006", format!("pop() takes at most 1 argument, got {}", args.len())).with_help("remove extra arguments"));
                }

                let index = if args.is_empty() {
                    -1
                } else {
                    match args[0] {
                        Value::Int(i) => i,
                        _ => return Err(self.error("R002", "pop index must be an integer")),
                    }
                };

                let mut borrowed_items = items.borrow_mut();
                let len = borrowed_items.len() as i64;

                if len == 0 {
                    return Err(self.error("R003", "pop from empty list"));
                }

                let effective_index = if index < 0 { len + index } else { index };
                if effective_index < 0 || effective_index >= len {
                    return Err(self.error("R003", "pop index out of range"));
                }

                let popped_value = borrowed_items.remove(effective_index as usize);

                self.push(popped_value);
                Ok(MethodCache::ListPop)
            }

            "remove" => {
                if args.len() != 1 {
                    let hint = if args.len() > 1 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("R006", format!("remove() takes exactly 1 argument, got {}", args.len())).with_help(hint));
                }

                let value_to_remove = &args[0];
                let mut borrowed_items = items.borrow_mut();

                if let Some(index) = borrowed_items.iter().position(|x| x == value_to_remove) {
                    borrowed_items.remove(index);
                    self.push(Value::None);

                    Ok(MethodCache::EMPTY)
                } else {
                    Err(self.error("R008", format!("'{}' is not in list", value_to_remove))
                        .with_help("try calling remove() with an element in the list")
                    )
                }
            }

            _ => Err(self.error("R004", format!("list object has no method '{}'", method))),
        }
    }

    fn invoke_string_method(&mut self, s: &str, method: &str, args: &[Value]) -> Result<MethodCache, VyprError> {
        match method {
            "startswith" | "endswith" => {
                if args.len() != 1 {
                    let hint = if args.len() > 1 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("R006", format!("{}() takes exactly 1 argument, got {}", method, args.len())).with_help(hint));
                }

                let prefix = match args[0].as_str() {
                    Some(p) => p,
                    None => return Err(self.error("R002", format!("{}() arg must be a string", method))),
                };

                let result = if method == "startswith" { s.starts_with(prefix) } else { s.ends_with(prefix) };

                self.push(Value::Bool(result));
                Ok(MethodCache::EMPTY)
            }

            "isascii" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "isascii() takes no arguments").with_help("remove the arguments"));
                }

                self.push(Value::Bool(s.is_ascii()));
                Ok(MethodCache::EMPTY)
            }

            "isupper" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "isupper() takes no arguments").with_help("remove the arguments"));
                }

                let is_upper = !s.is_empty() && s == s.to_uppercase() && s != s.to_lowercase();
                self.push(Value::Bool(is_upper));

                Ok(MethodCache::EMPTY)
            }

            "islower" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "islower() takes no arguments").with_help("remove the arguments"));
                }

                let is_lower = !s.is_empty() && s == s.to_lowercase() && s != s.to_uppercase();
                self.push(Value::Bool(is_lower));

                Ok(MethodCache::EMPTY)
            }

            "isnumeric" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "isnumeric() takes no arguments").with_help("remove the arguments"));
                }

                let is_numeric = !s.is_empty() && s.chars().all(char::is_numeric);
                self.push(Value::Bool(is_numeric));

                Ok(MethodCache::EMPTY)
            }

            "join" => {
                if args.len() != 1 {
                    let hint = if args.len() > 1 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("R006", format!("join() takes exactly 1 argument, got {}", args.len())).with_help(hint));
                }

                let joined = match &args[0] {
                    Value::List(items) => {
                        let borrowed = items.borrow();
                        let string: Vec<String> = borrowed.iter().map(|v| v.to_string()).collect();
                        string.join(s)
                    },

                    Value::Range(bounds) => {
                        let (start, stop) = **bounds;
                        let string_elements: Vec<String> = (start..stop).map(|v| v.to_string()).collect();
                        string_elements.join(s)
                    },
                    
                    val if val.get_type() == DataType::Str => {
                        let string_elements: Vec<String> = val.as_str().unwrap().chars().map(|c| c.to_string()).collect();
                        string_elements.join(s)
                    },
                    _ => return Err(self.error("R002", "join() expects an iterable")),
                };

                self.push(Value::make_string(&joined));
                Ok(MethodCache::EMPTY)
            }

            "replace" => {
                if args.len() < 2 || args.len() > 3 {
                    let hint = if args.len() > 3 { "remove extra arguments" } else { "'old' and 'new' arguments are required" };
                    return Err(self.error("R006", format!("replace() takes 2 or 3 arguments, got {}", args.len())).with_help(hint));
                }

                let old_val = match args[0].as_str() {
                    Some(str_val) => str_val,
                    _ => return Err(self.error("R002", "replace() 'old' argument must be a string")),
                };

                let new_val = match args[1].as_str() {
                    Some(str_val) => str_val,
                    _ => return Err(self.error("R002", "replace() 'new' argument must be a string")),
                };

                let replaced_string = if args.len() == 3 {
                    let count = match &args[2] {
                        Value::Int(i) => *i,
                        _ => return Err(self.error("R002", "replace() 'count' argument must be an integer")),
                    };

                    if count < 0 {
                        s.replace(old_val, new_val)
                    } else {
                        s.replacen(old_val, new_val, count as usize)
                    }
                } else {
                    s.replace(old_val, new_val)
                };

                self.push(Value::make_string(&replaced_string));
                Ok(MethodCache::EMPTY)
            }

            "strip" => {
                if args.len() > 1 {
                    return Err(self.error("R006", format!("strip() takes at most 1 argument, got {}", args.len())));
                }

                // optimization: 'trim()' returns a '&str' slice
                // which 'make_string()' supports
                let stripped = if args.is_empty() {
                    s.trim()
                } else {
                    match args[0].as_str() {
                        Some(chars_to_remove) => s.trim_matches(|c| chars_to_remove.contains(c)),
                        _ => return Err(self.error("R002", "strip() argument must be a string")),
                    }
                };

                self.push(Value::make_string(&stripped));
                Ok(MethodCache::EMPTY)
            }

            // optimization: pipe the iterator slices directly into 'make_string()'
            // prevents allocation of Vec<String>
            "split" => {
                if args.len() > 2 {
                    return Err(self.error("R006", format!("split() takes at most 2 arguments, got {}", args.len())));
                }

                let string_parts: Vec<Value> = if args.is_empty() {
                    s.split_whitespace().map(Value::make_string).collect()
                } else {
                    let separator = match args[0].as_str() {
                        Some(sep) => sep,
                        _ => return Err(self.error("R002", "split() separator must be a string")),
                    };

                    if args.len() == 2 {
                        let maxsplit = match &args[1] {
                            Value::Int(m) => *m,
                            _ => return Err(self.error("R002", "split() maxsplit must be an integer")),
                        };

                        if maxsplit < 0 {
                            s.split(separator).map(Value::make_string).collect()
                        } else {
                            s.splitn((maxsplit + 1) as usize, separator).map(Value::make_string).collect()
                        }
                    } else {
                        s.split(separator).map(Value::make_string).collect()
                    }
                };

                self.push(Value::List(Rc::new(RefCell::new(string_parts))));

                Ok(MethodCache::EMPTY)
            }

            // optimization: SSO fast path
            "lower" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "lower() takes no arguments"));
                }

                if s.len() <= 14 && s.is_ascii() {
                    let mut buf = [0u8; 14];
                    buf[..s.len()].copy_from_slice(s.as_bytes());
                    buf[..s.len()].make_ascii_lowercase();
                    
                    self.push(Value::InlineStr(s.len() as u8, buf))
                } else {
                    self.push(Value::make_string(&s.to_lowercase()));
                }

                Ok(MethodCache::StringLower)
            }

            // optimization: SSO fast path
            "upper" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "upper() takes no arguments"));
                }

                if s.len() <= 14 && s.is_ascii() {
                    let mut buf = [0u8; 14];
                    buf[..s.len()].copy_from_slice(s.as_bytes());
                    buf[..s.len()].make_ascii_uppercase();
                    
                    self.push(Value::InlineStr(s.len() as u8, buf))
                } else {
                    self.push(Value::make_string(&s.to_uppercase()));
                }

                Ok(MethodCache::StringUpper)
            }

            _ => Err(self.error("R004", format!("string object has no method '{}'", method))),
        }
    }

    fn invoke_dict_method(&mut self, dict: Rc<RefCell<HashMap<DictKey, Value>>>, method: &str, args: &[Value]) -> Result<MethodCache, VyprError> {
        match method {
            "clear" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "clear() takes no arguments").with_help("remove the arguments"));
                }

                dict.borrow_mut().clear();
                self.push(Value::None);

                Ok(MethodCache::EMPTY)
            }

            "copy" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "copy() takes no arguments").with_help("remove the arguments"));
                }

                let cloned_map = dict.borrow().clone();
                self.push(Value::Dict(Rc::new(RefCell::new(cloned_map))));

                Ok(MethodCache::EMPTY)
            }

            "get" => {
                if args.is_empty() || args.len() > 2 {
                    let hint = if args.is_empty() { "add the key argument" } else { "remove extra arguments" };
                    return Err(self.error("R006", format!("get() takes, at most, 2arguments, got {}", args.len())).with_help(hint));
                }

                let dict_key = match DictKey::from_value(&args[0]) {
                    Ok(k) => k,
                    Err(msg) => return Err(self.error("R002", msg)),
                };

                let borrowed = dict.borrow();
                if let Some(val) = borrowed.get(&dict_key) {
                    self.push(val.clone());
                } else if args.len() == 2 {
                    self.push(args[1].clone());
                } else {
                    self.push(Value::None);
                }

                Ok(MethodCache::EMPTY)
            }

            "pop" => {
                if args.is_empty() || args.len() > 2 {
                    let hint = if args.is_empty() { "add the key argument" } else { "remove extra arguments" };
                    return Err(self.error("R006", format!("pop() takes at most 2 arguments, got {}", args.len())).with_help(hint));
                }

                let dict_key = match DictKey::from_value(&args[0]) {
                    Ok(k) => k,
                    Err(msg) => return Err(self.error("R002", msg)),
                };

                let mut borrowed = dict.borrow_mut();
                if let Some(val) = borrowed.remove(&dict_key) {
                    self.push(val);
                } else if args.len() == 2 {
                    self.push(args[1].clone());
                } else {
                    return Err(self.error("R008", format!("key '{}' not found in dictionary", dict_key.to_value()))
                        .with_help("provide a default fallback value: pop(key, default)"));
                }

                Ok(MethodCache::EMPTY)
            }

            "keys" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "keys() takes no arguments").with_help("remove the arguments"));
                }
                
                let borrowed = dict.borrow();
                let keys_list: Vec<Value> = borrowed.keys()
                    .map(|k| k.to_value())
                    .collect();

                self.push(Value::List(Rc::new(RefCell::new(keys_list))));
                Ok(MethodCache::EMPTY)
            }

            "values" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "values() takes no arguments").with_help("remove the arguments"));
                }

                let borrowed = dict.borrow();
                let values_list: Vec<Value> = borrowed.values()
                    .cloned()
                    .collect();

                self.push(Value::List(Rc::new(RefCell::new(values_list))));
                Ok(MethodCache::EMPTY)
            }

            _ => Err(self.error("R004", format!("dict object has no method '{}'", method))),
        }
    }

    fn invoke_file_method(&mut self, file: SharedFile, method: &str, args: &[Value]) -> Result<MethodCache, VyprError> {
        use std::io::{Read, Write, BufRead, BufReader};

        let mut f = file.0.borrow_mut();

        match method {
            "read" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "read() takes no arguments").with_help("remove the arguments"));
                }

                let mut buf = String::new();
                if let Err(e) = f.read_to_string(&mut buf) {
                    return Err(self.error("R015", format!("failed to read file: {}", e)));
                }

                self.push(Value::make_string(&buf));
                Ok(MethodCache::EMPTY)
            }

            "write" => {
                if args.len() != 1 {
                    return Err(self.error("R006", format!("write() takes exactly 1 argument, got {}", args.len())));
                }

                let s = match args[0].as_str() {
                    Some(string) => string,
                    None => return Err(self.error("R002", "write() arguments must be a string")),
                };

                if let Err(e) = f.write_all(s.as_bytes()) {
                    return Err(self.error("R015", format!("failed to write to file: {}", e)));
                }

                self.push(Value::None);
                Ok(MethodCache::EMPTY)
            }

            "readlines" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "readlines() takes no arguments").with_help("remove the arguments"));
                }

                let reader = BufReader::new(&mut *f);
                let mut lines = Vec::new();

                for line in reader.lines() {
                    match line {
                        Ok(mut line) => {
                            line.push('\n');
                            lines.push(Value::make_string(&line));
                        }
                        Err(e) => return Err(self.error("R015", format!("failed to read line: {}", e))),
                    }
                }

                self.push(Value::List(Rc::new(RefCell::new(lines))));
                Ok(MethodCache::EMPTY)
            }

            "writelines" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "writelines() takes no arguments").with_help("remove the arguments"));
                }

                match &args[0] {
                    Value::List(items) => {
                        let borrowed = items.borrow();

                        for item in borrowed.iter() {
                            if let Some(s) = item.as_str() {
                                if let Err(e) = f.write_all(s.as_bytes()) {
                                    return Err(self.error("R015", format!("faield to write to file: {}", e)));
                                }
                            } else {
                                return Err(self.error("R002", "writelines() requires a list of strings"));
                            }
                        }
                    },
                    _ => return Err(self.error("R002", "writelines() requires a list of strings")),
                }

                self.push(Value::None);
                Ok(MethodCache::EMPTY)
            }

            "close" => {
                if !args.is_empty() {
                    return Err(self.error("R006", "close() takes no arguments").with_help("remove the arguments"));
                }

                let _ = f.flush();
                let _ = f.sync_all();

                self.push(Value::None);
                Ok(MethodCache::EMPTY)
            }

            _ => Err(self.error("R004", format!("file object has no method '{}'", method)))
        }
    }
}
