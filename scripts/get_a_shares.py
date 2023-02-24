import time

# import akshare as ak
#
# stock_zh_a_spot_em_df = ak.stock_zh_a_spot_em()

import tushare as ts

ts.set_token("d57609ffb2e6698a1cdc7ca4cdd1f62e3b4ae9e92194f344adf3f677")

pro = ts.pro_api()

data = pro.query(
    "stock_basic",
    exchange="",
    list_status="L",
    fields="ts_code,symbol,name,area,industry,list_date",
)

for code in data.ts_code:
    try:
        print(f">>> try to get: {code}")
        df = ts.pro_bar(
            ts_code=code, adj="hfq", start_date="20000101", end_date="20230201"
        )
        df.to_csv(f"../data/a-shares/{code}.csv", index=False)
        print(f">>> get {code} success!!")
    except Exception as e:
        print(f"[ERROR] {e}")

    time.sleep(1)
