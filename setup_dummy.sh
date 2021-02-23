modprobe dummy
ip link add dummy type dummy
ip link set dummy up
ip addr add 192.168.0.44 dev dummy

tc qdisc add dev dummy root netem rate 1mbit limit 20 delay 300ms loss 15%
