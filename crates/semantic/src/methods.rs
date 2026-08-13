use error::error::{Span, VyprError};
use lexer::token::TokenType;
use parser::ast::{Expr, TypeExpr};

use crate::analyzer::Analyzer;

impl Analyzer {

    pub(crate) fn method_call(&mut self, callee: &Expr, args: &[Expr], method: &str, span: Span) -> Result<TypeExpr, VyprError> {
        let callee_type = self.infer_type(callee)?;

        match callee_type {
            TypeExpr::List(inner) => self.check_list_method(&inner, method, args, span),
            TypeExpr::Atomic(TokenType::STR) => self.check_string_method(method, args, span),
            TypeExpr::Dict(key, value) => self.check_dict_method(&key, &value, method, args, span),
            TypeExpr::Any => Ok(TypeExpr::Any),

            t => Err(self.error("S009", format!("type {} has no method '{}'", t, method), span))
        }
    }

    fn check_list_method(&mut self, inner: &TypeExpr, method: &str, args: &[Expr], span: Span) -> Result<TypeExpr, VyprError> {
        match method {
            "append" => {
                if args.len() > 1 {
                    return Err(self.error("S006", "append() takes exactly 1 argument", span).with_help("remove the extra arguments"));
                }
                if args.is_empty() {
                    return Err(self.error("S006", "append() takes exactly 1 argument", span).with_help("add an argument"));
                }

                let arg_type = self.infer_type(&args[0])?;
                if !self.types_match(inner, &arg_type) {
                    return Err(self.error("S007", format!("type error: cannot append {} to list[{}]", arg_type, inner), span));
                }

                Ok(TypeExpr::Any)
            }

            "clear" => {
                if !args.is_empty() {
                    return Err(self.error("S006", "clear() takes no arguments", span).with_help("remove the arguments"))
                }

                Ok(TypeExpr::Any)
            }

            "insert" => {
                if args.len() != 2 {
                    let hint = if args.len() > 2 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("S006", format!("insert() takes exactly 2 arguments, got {}", args.len()), span).with_help(hint));
                }

                let index_arg_type = self.infer_type(&args[0])?;
                if !self.types_match(&TypeExpr::Atomic(TokenType::INT), &index_arg_type) {
                    return Err(self.error("S007", "type error: the index arg of insert() must be an integer", span));
                }

                let value_arg_type = self.infer_type(&args[1])?;
                if !self.types_match(inner, &value_arg_type) {
                    return Err(self.error(
                        "S007", 
                        format!("type error: cannot insert element of type '{}' into list[{}]", value_arg_type, inner), span)
                    );
                }

                Ok(TypeExpr::Any)
            }

            "pop" => {
                if args.len() > 1 {
                    return Err(self.error("S006", format!("pop() takes at most 1 argument, got {}", args.len()), span)
                        .with_help("remove extra arguments")
                    );
                }

                if args.len() == 1 {
                    let index_type = self.infer_type(&args[0])?;
                    if !self.types_match(&TypeExpr::Atomic(TokenType::INT), &index_type) {
                        return Err(self.error("S007", format!("type error: the index arg of pop() must be an integer, got {}", index_type), span))
                    }
                }

                Ok((*inner).clone())
            }

            "remove" => {
                if args.len() != 1 {
                    let hint = if args.len() > 1 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("S006", format!(
                        "remove() takes exactly 1 arguments, got {}", args.len()), 
                    span).with_help(hint));
                }

                let arg_type = self.infer_type(&args[0])?;
                if !self.types_match(inner, &arg_type) {
                    return Err(self.error("S007", format!(
                        "type error: list of type '{}' cannot hold elements of type '{}', therefore the element would not be in the list, thus irremovable", 
                        inner, arg_type
                    ), span));
                }

                Ok((*inner).clone())
            }

            _ => Err(self.error("S009", format!("list[{}] has no method '{}'", inner, method), span))
        }
    }

    fn check_string_method(&mut self, method: &str, args: &[Expr], span: Span) -> Result<TypeExpr, VyprError> {
        match method {
            "startswith" | "endswith" => {
                if args.len() != 1 {
                    let hint = if args.len() > 1 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("S006", format!("{}() takes exactly 1 arguments, got {}", method, args.len()), span).with_help(hint));
                }

                let arg_type = self.infer_type(&args[0])?;
                if !self.types_match(&TypeExpr::Atomic(TokenType::STR), &arg_type) {
                    return Err(self.error("S007", format!(
                        "type error: prefix must be of type 'str', got {}", arg_type), 
                    span).with_help("try casting to a str via str()"));
                }

                Ok(TypeExpr::Atomic(TokenType::BOOL))
            }

            "isascii" | "isupper" | "islower" | "isnumeric" => {
                if !args.is_empty() {
                    return Err(self.error("S006", format!("{}() takes no arguments", method), span).with_help("remove the arguments"));
                }

                Ok(TypeExpr::Atomic(TokenType::BOOL))
            }
            
            "lower" | "upper" => {
                if !args.is_empty() {
                    return Err(self.error("S006", format!("{}() takes no arguments", method), span).with_help("remove the arguments"));
                }

                Ok(TypeExpr::Atomic(TokenType::STR))
            }

            "join" => {
                if args.len() != 1 {
                    let hint = if args.len() > 1 { "remove extra arguments" } else { "add missing arguments" };
                    return Err(self.error("S006", format!("join() takes exactly 1 argument, got {}", args.len()), span).with_help(hint));
                }

                let arg_type = self.infer_type(&args[0])?;
                let is_iterable = matches!(arg_type, 
                    TypeExpr::List(_) | TypeExpr::Atomic(TokenType::RANGE) | 
                    TypeExpr::Atomic(TokenType::STR) | TypeExpr::Any
                );

                if !is_iterable {
                    return Err(self.error("S007", format!("type error: join() expects an iterable argument, got {}", arg_type), span));
                }

                Ok(TypeExpr::Atomic(TokenType::STR))
            }

            "replace" => {
                if args.len() < 2 || args.len() > 3 {
                    let hint = if args.len() > 3 { "remove extra arguments" } else { "add missing arguments ('old' and 'new' are required)" };
                    return Err(self.error("S006", format!("replace() takes 2 or 3 arguments, got {}", args.len()), span).with_help(hint));
                }

                let old_type = self.infer_type(&args[0])?;
                if !self.types_match(&TypeExpr::Atomic(TokenType::STR), &old_type) {
                    return Err(self.error("S007", format!("type error: 'old' argument of replace() must be a str, got {}", old_type), span));
                }

                let new_type = self.infer_type(&args[1])?;
                if !self.types_match(&TypeExpr::Atomic(TokenType::STR), &new_type) {
                    return Err(self.error("S007", format!("type error: 'new' argument of replace() must be a str, got {}", new_type), span));
                }

                if args.len() == 3 {
                    let count_type = self.infer_type(&args[2])?;

                    if !self.types_match(&TypeExpr::Atomic(TokenType::INT), &count_type) {
                        return Err(self.error("S007", format!("type error: 'count' argument of replace() must be an int, got {}", count_type), span));
                    }
                }

                Ok(TypeExpr::Atomic(TokenType::STR))
            }

            "split" => {
                if args.len() > 2 {
                    return Err(self.error("S006", format!("split() takes at most 2 arguments, got {}", args.len()), span)
                        .with_help("remove extra arguments")
                    );
                }

                if !args.is_empty() {
                    let sep_type = self.infer_type(&args[0])?;

                    if !self.types_match(&TypeExpr::Atomic(TokenType::STR), &sep_type) {
                        return Err(self.error("S007", format!(
                            "type error: 'separator' argument of split() must be a str, got {}", 
                            sep_type), span)
                        );
                    }
                }

                if args.len() == 2 {
                    let maxsplit_type = self.infer_type(&args[1])?;

                    if !self.types_match(&TypeExpr::Atomic(TokenType::INT), &maxsplit_type) {
                        return Err(self.error("S007", format!(
                            "type error: 'maxsplit' argument of split() must be an int, got {}",
                            maxsplit_type), span)
                        );
                    }
                }

                Ok(TypeExpr::List(Box::new(TypeExpr::Atomic(TokenType::STR))))
            }

            "strip" => {
                if args.len() > 1 {
                    return Err(self.error("S006", format!("strip() takes at most 1 argument, got {}", args.len()), span)
                        .with_help("remove extra arguments")
                    );
                }

                if args.len() == 1 {
                    let char_type = self.infer_type(&args[0])?;

                    if !self.types_match(&TypeExpr::Atomic(TokenType::STR), &char_type) {
                        return Err(self.error("S007", format!("type error: 'characters' argument of strip() must be a str, got {}", char_type), span));
                    }
                }

                Ok(TypeExpr::Atomic(TokenType::STR))
            }

            _ => Err(self.error("S009", format!("type 'str' has no method '{}'", method), span))
        }
    }

    fn check_dict_method(&mut self, key: &TypeExpr, value: &TypeExpr, method: &str, args: &[Expr], span: Span) 
        -> Result<TypeExpr, VyprError> 
    {
        match method {
            // removes all elements from the dictionary
            // takes: no parameters
            // returns: none
            "clear" => {
                if !args.is_empty() {
                    return Err(self.error("S006", "clear() takes no arguments", span).with_help("remove the arguments"));
                }

                Ok(TypeExpr::Any)
            }

            // returns a copy of the dictionary
            // takes: no parameters
            "copy" => {
                if !args.is_empty() {
                    return Err(self.error("S006", "copy() takes no arguments", span).with_help("remove the arguments"));
                }

                Ok(TypeExpr::Dict(Box::new((*key).clone()), Box::new((*value).clone())))
            }

            // returns the value of the specified key
            // takes:
            //      keyname: the keyname of the item
            //      value: optional value to return if key doesnt exist, default None
            "get" => {
                if args.is_empty() || args.len() > 2 {
                    let hint = if args.is_empty() { "add the key argument" } else { "remove extra arguments" };
                    return Err(self.error("S006", format!("get() takes at most 2 arguments, got {}", args.len()), span).with_help(hint));
                }

                let key_arg_type = self.infer_type(&args[0])?;
                if !self.types_match(key, &key_arg_type) {
                    return Err(self.error("S007", format!("type error: expected key of type {}, got {}", key, key_arg_type), span));
                }

                if args.len() == 2 {
                    let default_arg_type = self.infer_type(&args[1])?;

                    if self.types_match(value, &default_arg_type) {
                        Ok((*value).clone())
                    } else {
                        Ok(TypeExpr::Union(Box::new((*value).clone()), Box::new(default_arg_type)))
                    }
                } else {
                    Ok((*value).clone())
                }
            }

            // removes and returns the element with the specified key
            // takes:
            //      keyname: the keyname of the item to remove
            //      default: optional value to return if key does not exist
            //          - if this parameter isnt specified and key does not exist, throw error
            "pop" => {
                if args.is_empty() || args.len() > 2 {
                    let hint = if args.is_empty() { "add the key argument" } else { "remove extra arguments" };
                    return Err(self.error("S006", format!("pop() takes at most 2 arguments, got {}", args.len()), span).with_help(hint));
                }

                let key_arg_type = self.infer_type(&args[0])?;
                if !self.types_match(key, &key_arg_type) {
                    return Err(self.error("S007", format!("type error: expected key of type {}, got {}", key, key_arg_type), span));
                }

                if args.len() == 2 {
                    let default_arg_type = self.infer_type(&args[1])?;
                    if !self.types_match(value, &default_arg_type) {
                        return Ok(TypeExpr::Any);
                    }
                }

                Ok((*value).clone())
            }

            // returns a list containing all the keys of the dictionary
            // takes: no parameters
            "keys" => {
                if !args.is_empty() {
                    return Err(self.error("S006", "keys() takes no arguments", span).with_help("remove the arguments"));
                }

                Ok(TypeExpr::List(Box::new((*key).clone())))
            }

            // returns a list containing all the values of the dictionary
            // takes: no parameters
            "values" => {
                if !args.is_empty() {
                    return Err(self.error("S006", "values() takes no arguments", span).with_help("remove the arguments"));
                }

                Ok(TypeExpr::List(Box::new((*value).clone())))
            }

            _ => Err(self.error("S009", format!("type 'dict' has no method '{}'", method), span))
        }
    }
}
