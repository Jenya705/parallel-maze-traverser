import os

dir = "outputs"

vals = {}

for file in os.listdir(dir):
    with open(dir + "/" + file) as f:
        txt = f.read().split("\n")
        values = []
        for line in txt:
            if line.startswith("BFS time elapsed:"):
                time = line[len("BFS time elapsed:"):].strip()
                time_v = 0
                if time.endswith("ms"):
                    time_v = float(time[:-2]) * (10**3)
                elif time.endswith("µs"):
                    time_v = float(time[:-3])
                elif time.endswith("s"):
                    time_v = float(time[:-1]) * (10**6)
                else:
                    print("Failed")
                values.append(time_v)
        avg = sum(values) / len(values)
        if avg > (10**6):
            avg_s = str(int(avg / (10**6) * 1000) / 1000.0) + "s"
        elif avg > (10**3):
            avg_s = str(int(avg / (10**3) * 1000) / 1000.0) + "ms"
        else:
            avg_s = str(int(avg * 1000) / 1000.0) + "$\\upmu\\text{s}$"
        vals[file]=avg_s

for i in range(0, 10):
    print(i, end="")
    for suffix in ["output.st.txt", "outpu4t.mtcsbs.txt", "output4.mtabs.txt", "outpu8t.mtcsbs.txt", "output8.mtabs.txt"]:
        file = "labyrinthe" + str(i) + "." + suffix
        print("&", vals[file], end = "")
    print("\\\\ \\hline")