### ✅ How it works:

1. **Logs**

   * Stored inside a `logs` folder in your project:

     ```
     ./logs/server.log
     ./logs/server.pid
     ```

2. **Start server**

   ```bash
   ./run_server.sh
   ```

3. **Check logs**

   ```bash
   tail -f ./logs/server.log
   ```

4. **Stop server**

   ```bash
   kill $(cat ./logs/server.pid)
   ```


