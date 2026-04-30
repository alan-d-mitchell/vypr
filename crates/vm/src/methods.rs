use error::error::VyprError;

use crate::{value::Value, vm::VM};

impl VM {

    /*
    * Methods to implement (priority wise, eventually will implement all):
    *
    *   List:
    *       clear() -> removes all elements
    *       insert(index, value) -> adds an element at the given position 
    *       pop(index?) -> removes and returns the element at given index, default to end if no index
    *       remove(value) -> removes first element with given value
    *
    *   Strings:
    *       endswith(suffix) -> checks whether string ends with suffix
    *       startswith(prefix) -> checks whether string starts with prefix
    *       isascii() -> checks if ALL characters are ascii
    *       islower() -> checks if ALL characters are lower
    *       isupper() -> checks if ALL characters are upper
    *       lower() -> lowercases the entire string. returns new string
    *       upper() -> uppercases entire string. returns new string
    *       isnumeric() -> checks if ALL characters are numeric
    *       join(iterable) -> joins the given iterable into a string separated by the string
    *       .join() is being called on. for example: "".join(["hello", " world"])
    *       replace(old, new, count?) -> replaces the old string value with the new one. count
    *       is an optional integer that specifies the amount of occurences of old youd like to
    *       replace with new. default count is all
    *       split(separator?, maxsplit?) -> splits string into a list, optionally separated by
    *       the separator passed in. default separator is any whitespace. maxsplit is an
    *       integer that  determines how many splits to do. default is -1 or all occurence of
    *       separator
    *       strip(characters?) -> removes and leading and trailing whitespace. optional set of
    *       charcters to remove as trailing and leading characters. for example, txt =
    *       ",,,,,hello" txt.strip(",")
    */
    pub(crate) fn invoke_method(&mut self, name_idx: usize, arg_count: usize) -> Result<(), VyprError> {
        let method_name = self.read_string(name_idx)?;

        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            args.push(self.pop()?);
        }
        args.reverse();

        let obj = self.pop()?;
        
        match (obj, method_name.as_str()) {
            (Value::List(items), "append") => {
                if args.len() > 1 {
                    return Err(self.error("R006", "append() takes exactly 1 argument").with_help("remove the extra arguments"));
                }
                
                if args.is_empty() {
                    return Err(self.error("R006", "append() takes exactly 1 argument").with_help("add an argument"));
                }

                items.borrow_mut().push(args[0].clone()); // appends value to list

                self.push(Value::None);

                Ok(())
            }

            (Value::List(items), "clear") => {
                if !args.is_empty() {
                    return Err(self.error("R006", "clear() takes no arguments").with_help("remove the arguments"))
                }

                items.borrow_mut().clear();

                self.push(Value::None);

                Ok(())
            }

            (Value::List(items), "insert") => {
                if args.len() != 2 {
                    let hint = if args.len() > 2 { 
                        "remove extra arguments" 
                    } else { 
                        "add missing arguments" 
                    };

                    return Err(self.error("R006", format!(
                        "insert() takes exactly 2 arguments, got {}", args.len()), 
                    ).with_help(hint));
                }

                let index = match args[0] {
                    Value::Int(i) => i,
                    _ => return Err(self.error("R002", "insert() index must be an integer"))
                };

                let value = args[1].clone();
                let mut borrowed_items = items.borrow_mut();
                let len = borrowed_items.len() as i64;

                if index >= len {
                    borrowed_items.push(value); // index WAY out of bounds (positive)
                } else if index < 0 {
                    let effective_index = len + index;

                    if effective_index < 0 {
                        borrowed_items.insert(0, value); // index WAY out of bounds (negative)
                    } else {
                        // normal negative index => insert relative to end
                        borrowed_items.insert(effective_index as usize, value);
                    }
                } else {
                    // normal index, insert at index
                    borrowed_items.insert(index as usize, value);
                }

                self.push(Value::None);

                Ok(())
            }

            (Value::List(items), "pop") => {
                if args.len() > 1 {
                    return Err(self.error("R006", format!(
                        "pop() takes at most 1 argument, got {}", 
                        args.len()
                    )).with_help("remove extra arguments"));
                }

                let index = if args.is_empty() {
                    -1
                } else {
                    match args[0] {
                        Value::Int(i) => i,
                        _ => return Err(self.error("R002", "pop index must be an integer"))
                    }
                };

                let mut borrowed_items = items.borrow_mut();
                let len = borrowed_items.len() as i64;

                if len == 0 {
                    return Err(self.error("R003", "pop from empty list"));
                }

                let effective_index = if index < 0 {
                    len + index
                } else {
                    index
                };

                if effective_index < 0 || effective_index >= len {
                    return Err(self.error("R003", "pop index out of range"));
                }

                let popped_value = borrowed_items.remove(effective_index as usize);

                self.push(popped_value);

                Ok(())
            }

            (val, method) => {
                Err(self.error("R004", format!("object {:?} has no method '{}'", val.get_type(), method)))
            }
        }
    }
}
