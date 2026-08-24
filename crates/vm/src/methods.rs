use std::io::{Read, Write, BufRead, BufReader};
use error::error::VyprError;
use crate::{
    value::{ObjectDict, ObjectFile, ObjectList, ObjectRange, ObjectType, Value}, 
    vm::{MethodCache, VM}
};

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
        } else if obj.is_object() {
            unsafe {
                let raw_obj = obj.as_object();
                match (*raw_obj).ty {
                    ObjectType::LIST => self.invoke_list_method(&mut *(raw_obj as *mut ObjectList), &method_name, &args)?,
                    ObjectType::DICT => self.invoke_dict_method(&mut *(raw_obj as *mut ObjectDict), &method_name, &args)?,
                    ObjectType::FILE => self.invoke_file_method(&mut *(raw_obj as *mut ObjectFile), &method_name, &args)?,
                    _ => return Err(self.error("R004", format!("object {:?} has no method '{}'", obj.get_type(), method_name))),
                }
            }
        } else {
            return Err(self.error("R004", format!("object {:?} has no method '{}'", obj.get_type(), method_name)));
        };

        if cache_type != MethodCache::EMPTY {
            self.set_cache(current_ip, cache_type);
        }

        Ok(())
    }

    fn invoke_list_method(&mut self, list: &mut ObjectList, method: &str, args: &[Value]) -> Result<MethodCache, VyprError> {
        match method {
            "append" => {
                if args.len() > 1 { return Err(self.error("R006", "append() takes exactly 1 argument").with_help("remove the extra arguments")); }
                if args.is_empty() { return Err(self.error("R006", "append() takes exactly 1 argument").with_help("add an argument")); }

                list.items.push(args[0].clone());
                self.push(Value::none());
                Ok(MethodCache::ListAppend)
            }

            "clear" => {
                if !args.is_empty() { return Err(self.error("R006", "clear() takes no arguments").with_help("remove the arguments")); }
                list.items.clear();
                self.push(Value::none());
                Ok(MethodCache::EMPTY)
            }

            "insert" => {
                if args.len() != 2 {
                    let hint = if args.len() > 2 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("R006", format!("insert() takes exactly 2 arguments, got {}", args.len())).with_help(hint));
                }

                let index = if args[0].is_int() { args[0].as_int() as i64 } else { return Err(self.error("R002", "insert() index must be an integer")); };
                let value = args[1].clone();
                let len = list.items.len() as i64;

                if index >= len {
                    list.items.push(value);
                } else if index < 0 {
                    let effective_index = len + index;
                    if effective_index < 0 { list.items.insert(0, value); } else { list.items.insert(effective_index as usize, value); }
                } else {
                    list.items.insert(index as usize, value);
                }

                self.push(Value::none());
                Ok(MethodCache::EMPTY)
            }

            "pop" => {
                if args.len() > 1 { return Err(self.error("R006", format!("pop() takes at most 1 argument, got {}", args.len())).with_help("remove extra arguments")); }

                let index = if args.is_empty() { -1 } else if args[0].is_int() { args[0].as_int() as i64 } else { return Err(self.error("R002", "pop index must be an integer")); };
                let len = list.items.len() as i64;

                if len == 0 { return Err(self.error("R003", "pop from empty list")); }

                let effective_index = if index < 0 { len + index } else { index };
                if effective_index < 0 || effective_index >= len { return Err(self.error("R003", "pop index out of range")); }

                let popped_value = list.items.remove(effective_index as usize);
                self.push(popped_value);
                Ok(MethodCache::ListPop)
            }

            "remove" => {
                if args.len() != 1 {
                    let hint = if args.len() > 1 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("R006", format!("remove() takes exactly 1 argument, got {}", args.len())).with_help(hint));
                }

                let value_to_remove = &args[0];

                if let Some(index) = list.items.iter().position(|x| x == value_to_remove) {
                    list.items.remove(index);
                    self.push(Value::none());
                    Ok(MethodCache::EMPTY)
                } else {
                    Err(self.error("R008", format!("'{}' is not in list", value_to_remove)).with_help("try calling remove() with an element in the list"))
                }
            }

            _ => Err(self.error("R004", format!("list object has no method '{}'", method))),
        }
    }

    fn invoke_string_method(&mut self, s: &str, method: &str, args: &[Value]) -> Result<MethodCache, VyprError> {
        match method {
            "startswith" | "endswith" => {
                if args.len() != 1 { return Err(self.error("R006", format!("{}() takes exactly 1 argument, got {}", method, args.len()))); }
                let prefix = match args[0].as_str() {
                    Some(p) => p,
                    None => return Err(self.error("R002", format!("{}() arg must be a string", method))),
                };
                let result = if method == "startswith" { s.starts_with(prefix) } else { s.ends_with(prefix) };
                self.push(Value::boolean(result));
                Ok(MethodCache::EMPTY)
            }

            "isascii" => {
                if !args.is_empty() { return Err(self.error("R006", "isascii() takes no arguments")); }
                self.push(Value::boolean(s.is_ascii()));
                Ok(MethodCache::EMPTY)
            }

            "isupper" => {
                if !args.is_empty() { return Err(self.error("R006", "isupper() takes no arguments")); }
                let is_upper = !s.is_empty() && s == s.to_uppercase() && s != s.to_lowercase();
                self.push(Value::boolean(is_upper));
                Ok(MethodCache::EMPTY)
            }

            "islower" => {
                if !args.is_empty() { return Err(self.error("R006", "islower() takes no arguments")); }
                let is_lower = !s.is_empty() && s == s.to_lowercase() && s != s.to_uppercase();
                self.push(Value::boolean(is_lower));
                Ok(MethodCache::EMPTY)
            }

            "isnumeric" => {
                if !args.is_empty() { return Err(self.error("R006", "isnumeric() takes no arguments")); }
                let is_numeric = !s.is_empty() && s.chars().all(char::is_numeric);
                self.push(Value::boolean(is_numeric));
                Ok(MethodCache::EMPTY)
            }

            "join" => {
                if args.len() != 1 { return Err(self.error("R006", format!("join() takes exactly 1 argument, got {}", args.len()))); }

                let joined = if args[0].is_object() {
                    unsafe {
                        let obj = args[0].as_object();
                        match (*obj).ty {
                            ObjectType::LIST => {
                                let list = &*(obj as *const ObjectList);
                                let string_elements: Vec<String> = list.items.iter().map(|v| v.to_string()).collect();
                                string_elements.join(s)
                            }
                            ObjectType::RANGE => {
                                let r = &*(obj as *const ObjectRange);
                                let string_elements: Vec<String> = (r.start..r.stop).map(|v| v.to_string()).collect();
                                string_elements.join(s)
                            }
                            _ => return Err(self.error("R002", "join() expects an iterable")),
                        }
                    }
                } else if let Some(s_arg) = args[0].as_str() {
                    let string_elements: Vec<String> = s_arg.chars().map(|c| c.to_string()).collect();
                    string_elements.join(s)
                } else {
                    return Err(self.error("R002", "join() expects an iterable"));
                };

                let joined_str = Value::make_string(&mut self.heap, &joined);
                self.push(joined_str);
                Ok(MethodCache::EMPTY)
            }

            "replace" => {
                if args.len() < 2 || args.len() > 3 { return Err(self.error("R006", format!("replace() takes 2 or 3 arguments, got {}", args.len()))); }
                let old_val = match args[0].as_str() {
                    Some(str_val) => str_val,
                    _ => return Err(self.error("R002", "replace() 'old' argument must be a string")),
                };
                let new_val = match args[1].as_str() {
                    Some(str_val) => str_val,
                    _ => return Err(self.error("R002", "replace() 'new' argument must be a string")),
                };

                let replaced_string = if args.len() == 3 {
                    let count = if args[2].is_int() { args[2].as_int() } else { return Err(self.error("R002", "replace() 'count' argument must be an integer")); };
                    if count < 0 { s.replace(old_val, new_val) } else { s.replacen(old_val, new_val, count as usize) }
                } else {
                    s.replace(old_val, new_val)
                };

                let replaced = Value::make_string(&mut self.heap, &replaced_string);
                self.push(replaced);
                Ok(MethodCache::EMPTY)
            }

            "strip" => {
                if args.len() > 1 { return Err(self.error("R006", format!("strip() takes at most 1 argument, got {}", args.len()))); }
                let stripped = if args.is_empty() {
                    s.trim()
                } else {
                    match args[0].as_str() {
                        Some(chars_to_remove) => s.trim_matches(|c| chars_to_remove.contains(c)),
                        _ => return Err(self.error("R002", "strip() argument must be a string")),
                    }
                };

                let stripped_val = Value::make_string(&mut self.heap, stripped);
                self.push(stripped_val);
                Ok(MethodCache::EMPTY)
            }

            "split" => {
                if args.len() > 2 { return Err(self.error("R006", format!("split() takes at most 2 arguments, got {}", args.len()))); }
                let string_parts: Vec<Value> = if args.is_empty() {
                    s.split_whitespace().map(|part| Value::make_string(&mut self.heap, part)).collect()
                } else {
                    let separator = match args[0].as_str() {
                        Some(sep) => sep,
                        _ => return Err(self.error("R002", "split() separator must be a string")),
                    };
                    if args.len() == 2 {
                        let maxsplit = if args[1].is_int() { args[1].as_int() } else { return Err(self.error("R002", "split() maxsplit must be an integer")); };
                        if maxsplit < 0 {
                            s.split(separator).map(|part| Value::make_string(&mut self.heap, part)).collect()
                        } else {
                            s.splitn((maxsplit + 1) as usize, separator).map(|part| Value::make_string(&mut self.heap, part)).collect()
                        }
                    } else {
                        s.split(separator).map(|part| Value::make_string(&mut self.heap, part)).collect()
                    }
                };

                let list_val = Value::allocate_list(&mut self.heap, string_parts);
                self.push(list_val);
                Ok(MethodCache::EMPTY)
            }

            "lower" => {
                if !args.is_empty() { return Err(self.error("R006", "lower() takes no arguments")); }
                let lower_val = Value::make_string(&mut self.heap, &s.to_lowercase());
                self.push(lower_val);
                Ok(MethodCache::StringLower)
            }

            "upper" => {
                if !args.is_empty() { return Err(self.error("R006", "upper() takes no arguments")); }
                let upper_val = Value::make_string(&mut self.heap, &s.to_uppercase());
                self.push(upper_val);
                Ok(MethodCache::StringUpper)
            }

            _ => Err(self.error("R004", format!("string object has no method '{}'", method))),
        }
    }

    fn invoke_dict_method(&mut self, dict: &mut ObjectDict, method: &str, args: &[Value]) -> Result<MethodCache, VyprError> {
        match method {
            "clear" => {
                if !args.is_empty() { return Err(self.error("R006", "clear() takes no arguments")); }
                dict.items.clear();
                self.push(Value::none());
                Ok(MethodCache::EMPTY)
            }

            "copy" => {
                if !args.is_empty() { return Err(self.error("R006", "copy() takes no arguments")); }
                let cloned_map = dict.items.clone();
                let dict_val = Value::allocate_dict(&mut self.heap, cloned_map);
                self.push(dict_val);
                Ok(MethodCache::EMPTY)
            }

            "get" => {
                if args.is_empty() || args.len() > 2 { return Err(self.error("R006", format!("get() takes, at most, 2arguments, got {}", args.len()))); }
                if let Some(val) = dict.items.get(&args[0]) {
                    self.push(val.clone());
                } else if args.len() == 2 {
                    self.push(args[1].clone());
                } else {
                    self.push(Value::none());
                }
                Ok(MethodCache::EMPTY)
            }

            "pop" => {
                if args.is_empty() || args.len() > 2 { return Err(self.error("R006", format!("pop() takes at most 2 arguments, got {}", args.len()))); }
                if let Some(val) = dict.items.remove(&args[0]) {
                    self.push(val);
                } else if args.len() == 2 {
                    self.push(args[1].clone());
                } else {
                    return Err(self.error("R008", format!("key '{}' not found in dictionary", args[0])));
                }
                Ok(MethodCache::EMPTY)
            }

            "keys" => {
                if !args.is_empty() { return Err(self.error("R006", "keys() takes no arguments")); }
                let keys_list: Vec<Value> = dict.items.keys().cloned().collect();
                let list_val = Value::allocate_list(&mut self.heap, keys_list);
                self.push(list_val);
                Ok(MethodCache::EMPTY)
            }

            "values" => {
                if !args.is_empty() { return Err(self.error("R006", "values() takes no arguments")); }
                let values_list: Vec<Value> = dict.items.values().cloned().collect();
                let list_val = Value::allocate_list(&mut self.heap, values_list);
                self.push(list_val);
                Ok(MethodCache::EMPTY)
            }

            _ => Err(self.error("R004", format!("dict object has no method '{}'", method))),
        }
    }

    fn invoke_file_method(&mut self, file: &mut ObjectFile, method: &str, args: &[Value]) -> Result<MethodCache, VyprError> {
        let mut f = file.file.borrow_mut();

        match method {
            "read" => {
                if !args.is_empty() { return Err(self.error("R006", "read() takes no arguments")); }
                let mut buf = String::new();
                if let Err(e) = f.read_to_string(&mut buf) { return Err(self.error("R015", format!("failed to read file: {}", e))); }
                
                let string_val = Value::make_string(&mut self.heap, &buf);
                self.push(string_val);
                Ok(MethodCache::EMPTY)
            }

            "write" => {
                if args.len() != 1 { return Err(self.error("R006", format!("write() takes exactly 1 argument, got {}", args.len()))); }
                let s = match args[0].as_str() {
                    Some(string) => string,
                    None => return Err(self.error("R002", "write() arguments must be a string")),
                };
                if let Err(e) = f.write_all(s.as_bytes()) { return Err(self.error("R015", format!("failed to write to file: {}", e))); }
                
                self.push(Value::none());
                Ok(MethodCache::EMPTY)
            }

            "readlines" => {
                if !args.is_empty() { return Err(self.error("R006", "readlines() takes no arguments")); }
                let reader = BufReader::new(&mut *f);
                let mut lines = Vec::new();
                for line in reader.lines() {
                    match line {
                        Ok(mut line) => {
                            line.push('\n');
                            lines.push(Value::make_string(&mut self.heap, &line));
                        }
                        Err(e) => return Err(self.error("R015", format!("failed to read line: {}", e))),
                    }
                }
                
                let list_val = Value::allocate_list(&mut self.heap, lines);
                self.push(list_val);
                Ok(MethodCache::EMPTY)
            }

            "writelines" => {
                if !args.is_empty() { return Err(self.error("R006", "writelines() takes no arguments")); }
                if args[0].is_object() {
                    unsafe {
                        let obj = args[0].as_object();
                        if (*obj).ty == ObjectType::LIST {
                            let list = &*(obj as *const ObjectList);
                            for item in &list.items {
                                if let Some(s) = item.as_str() {
                                    if let Err(e) = f.write_all(s.as_bytes()) {
                                        return Err(self.error("R015", format!("failed to write to file: {}", e)));
                                    }
                                } else {
                                    return Err(self.error("R002", "writelines() requires a list of strings"));
                                }
                            }
                        } else {
                            return Err(self.error("R002", "writelines() requires a list of strings"));
                        }
                    }
                } else {
                    return Err(self.error("R002", "writelines() requires a list of strings"));
                }
                self.push(Value::none());
                Ok(MethodCache::EMPTY)
            }

            "close" => {
                if !args.is_empty() { return Err(self.error("R006", "close() takes no arguments")); }
                let _ = f.flush();
                let _ = f.sync_all();
                self.push(Value::none());
                Ok(MethodCache::EMPTY)
            }

            _ => Err(self.error("R004", format!("file object has no method '{}'", method)))
        }
    }
}
