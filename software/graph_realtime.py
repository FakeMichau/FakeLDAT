import subprocess
import matplotlib.pyplot as plt
from collections import deque
import time

show_graph = True
serial_cmd = ["stdbuf", "-oL", "cat", "/dev/ttyACM0"]

def calc_threshold(values: deque) :
    return sum(values) / len (values) * 0.95

last_button_state = 0
button_high_time = None  # Time when button went high
brightness_low_time = None  # Time when brightness went low
brightness_avg = deque(maxlen=30)

# For plotting
x_data = deque(maxlen=3000)
y1_data = deque(maxlen=3000)  # Inverse of Brightness
y2_data = deque(maxlen=3000)  # Button
plt.ion()
fig, ax = plt.subplots()
line1, = ax.plot(x_data, y1_data, label='Inverse of Brightness')
line2, = ax.plot(x_data, y2_data, label='Button')
ax.set_xlabel('Time')
ax.set_ylabel('Brightness')
ax.set_title('Real-time Graph')
ax.legend()
update_interval = 0.02
last_plot_time = time.time()

try:
    # Start subprocess to read from serial port
    serial_process = subprocess.Popen(serial_cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=1, universal_newlines=True)
    
    while True:
        line = serial_process.stdout.readline().strip()
        data = line.split(',')
        if len(data) == 3:
            try:
                x_data.append(data[0])  # Time
                brightness = float(data[1])
                button_state = float(data[2])
                y1_data.append(brightness)
                brightness_avg.append(brightness)

                scaled_value2 = button_state * 4095
                y2_data.append(scaled_value2)
                
                # Check for button transition
                if last_button_state != button_state:
                    if button_state == 1:
                        button_high_time = float(data[0])
                    last_button_state = button_state

                if brightness < calc_threshold(brightness_avg) and brightness_low_time is None and button_high_time is not None:
                    brightness_low_time = float(data[0])

                if brightness_low_time is not None:
                    print("Delay:", brightness_low_time - button_high_time, "Threshold: ", calc_threshold(brightness_avg))
                    button_high_time = None
                    brightness_low_time = None
                
                if show_graph and time.time() - last_plot_time > update_interval:
                    # Update plot
                    line1.set_xdata(range(len(x_data)))
                    line1.set_ydata(y1_data)
                    line2.set_xdata(range(len(x_data)))
                    line2.set_ydata(y2_data)
                    ax.relim()
                    ax.autoscale_view()
                    fig.canvas.draw()
                    fig.canvas.flush_events()
                    last_plot_time = time.time()
            except ValueError:
                print("Invalid data:", line)
except KeyboardInterrupt:
    serial_process.kill()  # Terminate subprocess
    print("Serial connection closed.")
