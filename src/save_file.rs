use std::{cell::RefCell, fs::File};
use std::io::prelude::*;
use std::rc::Rc;

use crate::game_object::{GameObject, SaveFileValue};

/// A function that reads a single character from a file
/// 
/// # Returns
/// 
/// The character read or None if the end of the file is reached
fn fgetc(file: &mut File) -> Option<char>{ // Shoutout to my C literate homies out there
    let mut buffer = [0; 1];
    let ret = file.read(&mut buffer);
    if ret.is_err() || ret.unwrap() == 0{
        return None;
    }
    return Some(buffer[0] as char);
}


/// A struct that represents a section in a ck3 save file
/// Each section has a name, and holds a file handle to the section
/// 
/// # Validity
/// 
/// The section is guaranteed to be valid when you get it.
/// However once you call [Section::to_object] or [Section::skip] the section becomes invalid.
/// Trying to do anything with an invalid section will panic.
/// 
/// # Example
/// 
/// ```
/// let save = SaveFile::new("save.ck3");
/// let section = save.next();
/// let object = section.to_object().unwrap();
/// ```
pub struct Section{
    name: String,
    file:Option<File>
}

impl Section{
    /// Create a new section
    fn new(name: &str, file: File) -> Section{
        Section{
            name: name.to_string(),
            file: Some(file)
        }
    }

    /// Get the name of the section
    pub fn get_name(&self) -> &str{
        &self.name
    }

    /// Invalidate the section
    fn invalidate(&mut self){
        self.file = None;
    }

    /// Convert the section to a GameObject and invalidate it.
    /// This is a rather costly process as it has to read the entire section contents and parse them.
    /// You can then make a choice if you want to parse the object or [Section::skip] it.
    /// The section must be valid.
    /// 
    /// # Panics
    /// 
    /// If the section is invalid
    pub fn to_object(&mut self) -> Option<GameObject>{
        if self.file.is_none(){
            panic!("Invalid section");
        }
        let mut quotes = false;
        let file = &mut self.file.as_mut().unwrap();
        // storage
        let mut key = String::new();
        let mut val = String::new();
        let mut past_eq = false; // we use this flag to determine if we are parsing a key or a value
        let mut depth = 0; // how deep we are in the object tree
        let mut object:GameObject = GameObject::new();
        //initialize the object stack
        let mut stack: Vec<GameObject> = Vec::new();
        stack.push(object);
        //initialize the key stack
        loop{
            let ret = fgetc(file);
            if ret.is_none(){
                break;
            }
            let c = ret.unwrap();
            match c{ // we parse the byte
                '{' => { // we have a new object, we push a new hashmap to the stack
                    depth += 1;
                    stack.push(GameObject::from_name(&key));
                    key.clear();
                    past_eq = false;
                }
                '}' => { // we have reached the end of an object
                    if past_eq {
                        if !val.is_empty() {
                            stack.last_mut().unwrap().insert(key.clone(), SaveFileValue::String(Rc::new(RefCell::new(val.clone()))));
                            key.clear();
                            val.clear();
                            past_eq = false;
                        }
                    }
                    else {
                        if !key.is_empty() {
                            stack.last_mut().unwrap().push(SaveFileValue::String(Rc::new(RefCell::new(key.clone()))));
                            key.clear();
                        }
                    }
                    depth -= 1;
                    if depth == 0{ // we have reached the end of the object we are parsing, we return the object
                        break;
                    }
                    else if depth < 0 { // we have reached the end of the file
                        panic!("Depth is negative");
                    }
                    else{ // we pop the inner object and insert it into the outer object
                        let inner = stack.pop().unwrap();
                        stack.last_mut().unwrap().insert(inner.get_name().to_string(), SaveFileValue::Object(Rc::new(RefCell::new(inner))));
                    }
                }
                '"' => { // we have a quote, we toggle the quotes flag
                    quotes = !quotes;
                }
                '\n' => { // we have reached the end of a line, we check if we have a key value pair
                    if past_eq { // we have a key value pair
                        stack.last_mut().unwrap().insert(key.clone(), SaveFileValue::String(Rc::new(RefCell::new(val.clone()))));
                        key.clear();
                        val.clear();
                        past_eq = false; // we reset the past_eq flag
                    }
                    else if !key.is_empty(){ // we have just a key { \n key \n }
                        stack.last_mut().unwrap().push(SaveFileValue::String(Rc::new(RefCell::new(key.clone()))));
                        key.clear();
                    }
                }
                ' ' | '\t' => { //syntax sugar we ignore, most of the time, unless...
                    if !quotes{ // we are not in quotes, we check if we have a key value pair
                        // we are key=value <-here
                        if past_eq && !val.is_empty() { // in case {something=else something=else}
                            stack.last_mut().unwrap().insert(key.clone(), SaveFileValue::String(Rc::new(RefCell::new(val.clone()))));
                            key.clear();
                            val.clear();
                            past_eq = false;
                        }
                        //
                        else if !key.is_empty() && !past_eq{ // in case { something something something } we want to preserve the spaces
                            stack.last_mut().unwrap().push(SaveFileValue::String(Rc::new(RefCell::new(key.clone()))));
                            key.clear();
                        }
                    }
                } 
                '=' => {
                    // if we have an assignment, we toggle adding from key to value
                    past_eq = true;
                }
                _ => { //the main content we append to the key or value
                    if past_eq{
                        val.push(c);
                    }else{
                        key.push(c);
                    }
                }
            }
        }
        object = stack.pop().unwrap();
        object.rename(self.name.clone());
        self.invalidate();
        if object.is_empty(){
            return None;
        }
        return Some(object);
    }

    /// Skip the current section and invalidate it.
    /// This is a rather cheap operation as it only reads the file until the end of the section.
    /// The section must be valid.
    /// 
    /// # Panics
    /// 
    /// If the section is invalid
    pub fn skip(&mut self){
        if self.file.is_none(){
            panic!("Invalid section");
        }
        let mut depth = 0;
        let file = &mut self.file.as_mut().unwrap();
        loop{
            let ret = fgetc(file);
            if ret.is_none(){
                break;
            }
            let c = ret.unwrap();
            match c{
                '{' => {
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0{
                        break;
                    }
                }
                _ => {}
            }
        }
        self.invalidate();
    }
}

/// A struct that represents a ck3 save file.
/// This struct is an iterator that returns sections from the save file.
/// 
/// # Example
/// 
/// ```
/// let save = SaveFile::new("save.ck3");
/// for section in save{
///    println!("Section: {}", section.get_name());
/// }
pub struct SaveFile{
    file: File
}

impl SaveFile{

    /// Create a new SaveFile instance.
    /// The filename must be valid of course.
    pub fn new(filename: &str) -> SaveFile{
        SaveFile{
            file: File::open(filename).unwrap(),
        }
    }

}

impl Iterator for SaveFile{

    type Item = Section;

    /// Get the next object in the save file
    /// If the file pointer has reached the end of the file then it will return None.
    fn next(&mut self) -> Option<Section>{
        let mut key = String::new();
        let file = &mut self.file;
        loop{
            let ret = fgetc(file);
            if ret.is_none(){
                return None;
            }
            let c = ret.unwrap();
            match c{
                '{' | '}' | '"' => {
                    panic!("Unexpected character");
                }
                '=' => {
                    break;
                }
                '\n' => {
                    key.clear();
                }
                ' ' | '\t' => {
                    continue;
                }
                _ => {
                    key.push(c);
                }
            }
            
        }
        return Some(Section::new(&key, self.file.try_clone().unwrap()));
    }
}

mod tests {

    #[allow(unused_imports)]
    use std::io::Write;

    #[allow(unused_imports)]
    use tempfile::NamedTempFile;

    #[allow(unused_imports)]
    use crate::game_object::GameObject;

    #[test]
    fn test_save_file(){
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"
            test={
                test2={
                    test3=1
                }
            }
        ").unwrap();
        let mut save_file = super::SaveFile::new(file.path().to_str().unwrap());
        let object = save_file.next().unwrap().to_object().unwrap();
        assert_eq!(object.get_name(), "test".to_string());
        let test2 = object.get_object_ref("test2");
        let test3 = test2.get_string_ref("test3");
        assert_eq!(*(test3) , "1".to_string());
    }

    #[test]
    fn test_save_file_array(){
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"
            test={
                test2={
                    1
                    2
                    3
                }
                test3={ 1 2 3}
            }
        ").unwrap();
        let mut save_file = super::SaveFile::new(file.path().to_str().unwrap());
        let object = save_file.next().unwrap().to_object().unwrap();
        assert_eq!(object.get_name(), "test".to_string());
        let test2 = object.get_object_ref("test2");
        let test2_val = test2;
        assert_eq!(*(test2_val.get_index(0).unwrap().as_string_ref().unwrap()) , "1".to_string());
        assert_eq!(*(test2_val.get_index(1).unwrap().as_string_ref().unwrap()) , "2".to_string());
        assert_eq!(*(test2_val.get_index(2).unwrap().as_string_ref().unwrap()) , "3".to_string());
        let test3 = object.get_object_ref("test3");
        let test3_val = test3;
        assert_eq!(*(test3_val.get_index(0).unwrap().as_string_ref().unwrap()) , "1".to_string());
        assert_eq!(*(test3_val.get_index(1).unwrap().as_string_ref().unwrap()) , "2".to_string());
        assert_eq!(*(test3_val.get_index(2).unwrap().as_string_ref().unwrap()) , "3".to_string());
    }

    #[test]
    fn test_weird_syntax(){
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"
            test={
                test2={1=2
                    3=4}
                test3={1 2 
                    3}
                test4={1 2 3}
                test5=42
            }
        ").unwrap();
        let mut save_file = super::SaveFile::new(file.path().to_str().unwrap());
        let object = save_file.next().unwrap().to_object().unwrap();
        assert_eq!(object.get_name(), "test".to_string());
        let test2 = object.get_object_ref("test2");
        assert_eq!(*(test2.get_string_ref("1")) , "2".to_string());
        assert_eq!(*(test2.get_string_ref("3")) , "4".to_string());
    }

    #[test]
    fn test_array_syntax(){
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"
            test={
                test2={ 1 2 3 }
            }
        ").unwrap();
        let mut save_file = super::SaveFile::new(file.path().to_str().unwrap());
        let object = save_file.next().unwrap().to_object().unwrap();
        assert_eq!(object.get_name(), "test".to_string());
        let test2 = object.get_object_ref("test2");
        assert_eq!(*(test2.get_index(0).unwrap().as_string_ref().unwrap()) , "1".to_string());
        assert_eq!(*(test2.get_index(1).unwrap().as_string_ref().unwrap()) , "2".to_string());
        assert_eq!(*(test2.get_index(2).unwrap().as_string_ref().unwrap()) , "3".to_string());
        assert_eq!(test2.get_array_iter().len(), 3);
    }

    #[test]
    fn test_example_1(){
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"
        3623={
            name=\"dynn_Sao\"
            variables={
                data={ {
                        flag=\"ai_random_harm_cooldown\"
                        tick=7818
                        data={
                            type=boolean
                            identity=1
                        }
        
                    }
         }
            }
            found_date=750.1.1
            head_of_house=83939093
            dynasty=3623
            historical={ 4440 5398 6726 10021 33554966 50385988 77977 33583389 50381158 50425637 16880568 83939093 }
            motto={
                key=\"motto_with_x_I_seek_y\"
                variables={ {
                        key=\"1\"
                        value=\"motto_the_sword_word\"
                    }
         {
                        key=\"2\"
                        value=\"motto_bravery\"
                    }
         }
            }
            artifact_claims={ 83888519 }
        }").unwrap();
        let mut save_file = super::SaveFile::new(file.path().to_str().unwrap());
        let object = save_file.next().unwrap().to_object().unwrap();
        assert_eq!(object.get_name(), "3623".to_string());
        assert_eq!(*(object.get_string_ref("name")) , "dynn_Sao".to_string());
        let historical = object.get_object_ref("historical");
        assert_eq!(*(historical.get_index(0).unwrap().as_string_ref().unwrap()) , "4440".to_string());
        assert_eq!(*(historical.get_index(1).unwrap().as_string_ref().unwrap()) , "5398".to_string());
        assert_eq!(*(historical.get_index(2).unwrap().as_string_ref().unwrap()) , "6726".to_string());
        assert_eq!(*(historical.get_index(3).unwrap().as_string_ref().unwrap()) , "10021".to_string());
        assert_eq!(*(historical.get_index(4).unwrap().as_string_ref().unwrap()) , "33554966".to_string());
        assert_eq!(historical.get_array_iter().len(), 12);
    }
}
