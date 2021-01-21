from sanic import Sanic
from sanic.response import json


app = Sanic("Test")


@app.route("/")
async def test(request):
    return json({"hello": "world"})


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5001, debug=False, access_log=False)
