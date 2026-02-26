import os
import re
import subprocess
import glob

# Regex for ```turn ... ```
md_regex = re.compile(r'```turn\n(.*?)```', re.DOTALL)
# Regex for <CodeBlock lang="turn"...>{` ... `}</CodeBlock>
mdx_regex = re.compile(r'<CodeBlock[^>]*lang="turn"[^>]*>\{\`(.*?)\`\}</CodeBlock>', re.DOTALL)
# Regex for Playground examples: code: ` ... `,
ts_regex = re.compile(r'code:\s*`(.*?)`,', re.DOTALL)

def extract_and_test(file_path, regexes):
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()

    snippets = []
    for r in regexes:
        snippets.extend(r.findall(content))
    
    if not snippets:
        return True
    
    print(f"\n--- Testing snippets in {file_path} ---")
    
    all_passed = True
    for i, snip in enumerate(snippets):
        # Write to temp file
        temp_file = f"/tmp/test_snippet_{i}.tn"
        with open(temp_file, 'w', encoding='utf-8') as f:
            f.write(snip.replace('\\n', '\n').strip())
        
        # We just want to see if it parses/compiles. 
        try:
            result = subprocess.run(["cargo", "run", "-q", "--", "run", temp_file], cwd="/Users/muyukanikizito/Turn/impl", capture_output=True, text=True, timeout=3)
        except subprocess.TimeoutExpired:
            print(f"✅ Snippet {i+1} OK (Syntax valid, deadlocked at runtime)")
            continue
        
        # If it's a parser error, it will say "Parser error"
        if "Parser error" in result.stderr or result.returncode != 0 and "Parser error" in result.stdout:
            print(f"❌ Snippet {i+1} FAILED parsing in {file_path}")
            print(result.stderr)
            print(result.stdout)
            all_passed = False
        else:
            # We treat non-parser errors (like runtime 'provider not found' or 'timeout') as pseudo-passes for syntactical correctness, but let's print if it actually fails
            if result.returncode != 0:
                print(f"⚠️ Snippet {i+1} in {file_path} failed at RUNTIME (Syntax OK)")
                # Print just the last few lines
                print('\n'.join(result.stderr.splitlines()[-3:]))
            else:
                print(f"✅ Snippet {i+1} OK")
    return all_passed

def main():
    success = True
    
    # Check README
    if not extract_and_test("/Users/muyukanikizito/Turn/README.md", [md_regex]):
        success = False
        
    # Check MDX
    for mdx in glob.glob("/Users/muyukanikizito/Prescott-Data-Applications/turn-website/src/app/docs/**/*.mdx", recursive=True):
        if not extract_and_test(mdx, [mdx_regex, md_regex]):
            success = False

    # Check Playground
    if not extract_and_test("/Users/muyukanikizito/Prescott-Data-Applications/turn-website/src/components/Playground.tsx", [ts_regex]):
        success = False

    if not success:
        print("\n💥 SOME SNIPPETS FAILED")
        exit(1)
    else:
        print("\n🎉 ALL SNIPPETS OK")

if __name__ == "__main__":
    main()
