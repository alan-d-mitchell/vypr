use error::error::{Span, VyprError};
use lexer::token::TokenType;
use parser::ast::{Expr, TypeExpr};

use crate::analyzer::Analyzer;

impl Analyzer {

    pub(crate) fn method_call(&mut self, callee: &Expr, args: &[Expr], method: &str, span: Span) -> Result<TypeExpr, VyprError> {
        let callee_type = self.infer_type(callee)?;

        /*
        * Methods to type check (priority wise, eventually will implement all):
        *
        *   List:
        *       clear() -> removes all elements (no type checking needed)
        *       insert(index, value) -> adds an element at the given position. shifts elements based
        *       on the index passed
        *           - index = a number specifying the position
        *           - value = if list is annotated, value MUST be of type(s) in annotation.
        *           otherwise, value does not require type checking
        *               - for example:
        *                   arr: list[int | str] = [1, 2, 3, "4"]
        *                   arr.insert(2, 2.0) -> would error
        *               - but:
        *                   arr = [1, 2, 3, "4"]
        *                   arr.insert(2, 2.0) -> would not error
        *       pop(index?) -> removes and returns the element at given index, default to end if no index
        *           - index = optional integer, defailt is -1 which is the last element of the list
        *       remove(value) -> removes first element with given value
        *           - if value does not exist, throw error saying "'{}' is not in list", value
        *           - since python says value can be of any type, so can we but if the list is
        *           annotated and the value is of a type that the list cannot hold, we will error
        *           during semantic saying something like: "list cannot hold elements of type
        *           'value'. so there is no way it would be removable" though this is more of a 
        *           warning, if no annotation, only error if the value doesnt exist
        *
        *   Strings:
        *       endswith(suffix) -> checks whether string ends with suffix
        *           - verify suffix is of type 'str'
        *       startswith(prefix) -> checks whether string starts with prefix
        *           - verify prefix is of type 'str'
        *       isascii() -> checks if ALL characters are ascii
        *           - no type checking needed
        *       islower() -> checks if ALL characters are lower
        *           - no type checking needed
        *       isupper() -> checks if ALL characters are upper
        *           - no type checking needed
        *       lower() -> lowercases the entire string. returns new string
        *           - no type checking needed
        *       upper() -> uppercases entire string. returns new string
        *           - no type checking needed
        *       isnumeric() -> checks if ALL characters are numeric
        *           - no type checking needed
        *       join(iterable) -> joins the given iterable into a string separated by the string
        *       .join() is being called on. for example: "".join(["hello", " world"])
        *           - iterable = needs to be any type that is iterable
        *               - can be list[any], range(5), etc etc
        *       replace(old, new, count?) -> replaces the old string value with the new one. count
        *       is an optional integer that specifies the amount of occurences of old youd like to
        *       replace with new. default count is all
        *           - verify old is of type str
        *           - verify new is of type str
        *           - count = a number specifying amount of occurences to replace. default = all
        *           occurences
        *       split(separator?, maxsplit?) -> splits string into a list, optionally separated by
        *       the separator passed in. default separator is any whitespace. maxsplit is an
        *       integer that determines how many splits to do. default is -1 or all occurence of
        *       separator
        *           - verify separator, if passed, is a str
        *           - verify maxsplit, if passed, is an integer
        *       strip(characters?) -> removes and leading and trailing whitespace. optional set of
        *       charcters to remove as trailing and leading characters. for example, txt =
        *       ",,,,,hello" txt.strip(",")
        *           - verify characters, if passed, is a str
        */
        match (callee_type, method) {
            
            (TypeExpr::List(inner), "append") => {
                if args.len() > 1 {
                    return Err(self.error("S006", "append() takes exactly 1 argument", span).with_help("remove the extra arguments"));
                }
                
                if args.is_empty() {
                    return Err(self.error("S006", "append() takes exactly 1 argument", span).with_help("add an argument"));
                }

                let arg_type = self.infer_type(&args[0])?;

                if !self.types_match(&inner, &arg_type) {
                    return Err(self.error("S007", format!(
                        "type error: cannot append {} to list[{}]",
                        arg_type, inner
                    ), span));
                }

                Ok(TypeExpr::Any)
            }

            (TypeExpr::List(_), "clear") => {
                if !args.is_empty() {
                    return Err(self.error("S006", "clear() takes no arguments", span).with_help("remove the arguments"))
                }

                Ok(TypeExpr::Any)
            }

            (TypeExpr::List(inner), "insert") => {
                if args.len() != 2 {
                    let hint = if args.len() > 2 { 
                        "remove extra arguments" 
                    } else { 
                        "add missing arguments" 
                    };

                    return Err(self.error("S006", format!(
                        "insert() takes exactly 2 arguments, got {}", args.len()), 
                        span
                    ).with_help(hint));
                }

                let index_arg_type = self.infer_type(&args[0])?;

                if !self.types_match(&TypeExpr::Atomic(TokenType::INT), &index_arg_type) {
                    return Err(self.error("S007",
                        "type error: the index arg of insert() must be an integer",
                        span
                    ));
                }

                let value_arg_type = self.infer_type(&args[1])?;

                if !self.types_match(&inner, &value_arg_type) {
                    return Err(self.error("S007", format!(
                        "type error: cannot insert element of type '{}' into list[{}]",
                        value_arg_type, inner
                    ), span));
                }
                
                Ok(TypeExpr::Any)
            }

            (TypeExpr::List(inner), "pop") => {
                if args.len() > 1 {
                    return Err(self.error("S006", format!(
                        "pop() takes at most 1 argument, got {}", 
                        args.len()
                    ), span).with_help("remove extra arguments"));
                }

                if args.len() == 1 {
                    let index_type = self.infer_type(&args[0])?;

                    if !self.types_match(&TypeExpr::Atomic(TokenType::INT), &index_type) {
                        return Err(self.error("S007", format!(
                            "type error: the index arg of pop() must be an integer, got {}", 
                            index_type
                        ), span))
                    }
                }

                Ok((*inner).clone())
            }

            (TypeExpr::Any, _) => {
                Ok(TypeExpr::Any)
            }

            (t, m) => Err(self.error("S009", format!("type {} has no method '{}'", t, m), span))
        }
    }
}
