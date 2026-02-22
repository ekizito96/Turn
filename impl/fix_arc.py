import os
import re
import glob

def fix_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()
    
    lines = content.split('\n')
    new_lines = []
    
    for line in lines:
        # We only want to transform Value::Str(expr) if it's NOT a pattern match.
        # Simple heuristic: if the line has `=>` or `if let` or `match`, be careful.
        # But actually, `expr` can be anything.
        # Let's just do a naive regex replacement and then run `rustfmt` and fix any remaining errors manually.
        
        # Replace `Value::Str("foo".to_string())`
        line = re.sub(r'Value::Str\((.*?\.to_string\(\))\)', r'Value::Str(std::sync::Arc::new(\1))', line)
        line = re.sub(r'Value::Str\((format!.*?)\)', r'Value::Str(std::sync::Arc::new(\1))', line)
        line = re.sub(r'Value::List\((vec!\[.*?\])\)', r'Value::List(std::sync::Arc::new(\1))', line)
        line = re.sub(r'Value::Map\((IndexMap::new\(\))\)', r'Value::Map(std::sync::Arc::new(\1))', line)
        line = re.sub(r'Value::Map\((map)\)', r'Value::Map(std::sync::Arc::new(\1))', line)
        line = re.sub(r'Value::Struct\((.*?)\s*,\s*(IndexMap::new\(\))\)', r'Value::Struct(std::sync::Arc::new(\1), std::sync::Arc::new(\2))', line)
        line = re.sub(r'Value::Struct\((.*?)\s*,\s*(map)\)', r'Value::Struct(std::sync::Arc::new(\1), std::sync::Arc::new(\2))', line)
        line = re.sub(r'Value::Struct\((.*?)\s*,\s*(struct_map)\)', r'Value::Struct(std::sync::Arc::new(\1), std::sync::Arc::new(\2))', line)
        line = re.sub(r'Value::List\((list)\)', r'Value::List(std::sync::Arc::new(\1))', line)
        
        # Also fix `.clone()` calls if they are matching a Value::Str(s) and returning it
        # Sometimes it's `Value::Str(s.clone())`
        line = re.sub(r'Value::Str\((.*?\.clone\(\))\)', r'Value::Str(std::sync::Arc::new(\1))', line)
        line = re.sub(r'Value::Str\((.*?\.to_owned\(\))\)', r'Value::Str(std::sync::Arc::new(\1))', line)
        
        # Handle string literals that need .to_string()
        # line = re.sub(r'Value::Str\("([^"]*)"\)', r'Value::Str(std::sync::Arc::new("\1".to_string()))', line)
        
        new_lines.append(line)
        
    with open(filepath, 'w') as f:
        f.write('\n'.join(new_lines))

if __name__ == '__main__':
    for f in glob.glob('src/**/*.rs', recursive=True):
        fix_file(f)
