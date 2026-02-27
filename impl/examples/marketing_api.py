import json
import subprocess
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
import uvicorn
import re

app = FastAPI(
    title="Turn Marketing Agency API",
    description="A production wrapper for the Turn Marketing Agents, demonstrating CLI interop."
)

class CampaignRequest(BaseModel):
    product_name: str

class CampaignResponse(BaseModel):
    campaign_id: float
    product: str
    seo_keywords: str
    approved: bool
    final_copy: str
    conf_score: float

@app.post("/api/v1/campaign", response_model=CampaignResponse)
async def generate_campaign(req: CampaignRequest):
    """
    Kicks off a Turn Marketing Agency run for the specified product.
    The Python backend shells out to the compiled Turn VM, passes the
    product brief via stdin to the actor's mailbox, and parses the result.
    """
    print(f"--> Dispatched Turn Agent for: {req.product_name}")
    
    # In a real deployed environment, TURN_INFER_PROVIDER and OPENAI_API_KEY
    # must be set in the environment variables where this worker runs.
    
    try:
        # We use the built-in standard CLI command:
        # `turn run <file> --id <session>`
        # and we send the `req.product_name` directly to the stdin stream,
        # which Turn's `receive` primitive can optionally intercept if the agent
        # chooses to read from its external mailbox.
        process = subprocess.Popen(
            ["cargo", "run", "--quiet", "--", "run", "impl/examples/marketing_agency.tn", "--id", "mktg_agency_cluster"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        # Send the product name as the mailbox message
        stdout, stderr = process.communicate(input=req.product_name)
        
        if process.returncode != 0:
            print("Turn VM Error:", stderr)
            raise HTTPException(status_code=500, detail="Turn Agent execution failed.")
            
        # The agent's STDOUT contains all tracing and echo commands.
        # We parse the clean JSON payload bounded by our API markers.
        match = re.search(r'=== API_RESPONSE_START ===\n(.*?)\n=== API_RESPONSE_END ===', stdout, re.DOTALL)
        if not match:
            print("Turn raw output:", stdout)
            raise HTTPException(status_code=500, detail="Failed to parse Turn agent response format.")
            
        json_payload = match.group(1).strip()
        data = json.loads(json_payload)
        
        return data

    except json.JSONDecodeError:
        raise HTTPException(status_code=500, detail="Turn Agent returned invalid JSON.")
    except FileNotFoundError:
        raise HTTPException(status_code=500, detail="Cargo/Turn executable not found in path.")

if __name__ == "__main__":
    print("Starting Turn Marketing API wrapper on port 8000...")
    print("Run test: curl -X POST http://localhost:8000/api/v1/campaign -H 'Content-Type: application/json' -d '{\"product_name\": \"Smart Coffee Maker\"}'")
    uvicorn.run(app, host="0.0.0.0", port=8000)
