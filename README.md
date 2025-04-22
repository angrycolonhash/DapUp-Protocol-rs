
# Process

Create 2 threads, one to advertise and one to scan  

Once another device has been located, it will stop the advertise and scan threads.
The MCU will setup a server and a client using another 2 threads
Both devices will connect to each other and transfer information. 

After a final confirmation of non-corrupted correct data (ACK_OK), 
it will add that device's MAC address to a blocklist (stopping connecting)
before disconnecting from each other and looking for other users. (starting cycle over again)

Blocklist can be edited through web server (for now, until displays can get working).
Blocklists contain the device MAC address and the information about the device + user info.
Blocklists only stop the MCU from connecting back to the device again, it is not a permanent thing