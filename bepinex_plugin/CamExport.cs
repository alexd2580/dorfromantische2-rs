using BepInEx;
using UnityEngine;
using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;

namespace CamExport
{
    [BepInPlugin("com.solver.camexport", "CamExport", "1.0.0")]
    public class CamExport : BaseUnityPlugin
    {
        private string filePath;
        private TcpListener listener;
        private Thread serverThread;

        void Awake()
        {
            // Write to a file in the game directory.
            filePath = Path.Combine(Paths.GameRootPath, "camera_pos.txt");
            Logger.LogInfo("CamExport loaded, writing to " + filePath);

            // Also start a simple TCP server on port 9123.
            serverThread = new Thread(RunServer);
            serverThread.IsBackground = true;
            serverThread.Start();
        }

        void LateUpdate()
        {
            var cam = Camera.main;
            if (cam == null) return;

            var pos = cam.transform.position;
            var rot = cam.transform.eulerAngles;
            var fov = cam.fieldOfView;

            var line = string.Format("{0:F4} {1:F4} {2:F4} {3:F4} {4:F4} {5:F4} {6:F4}",
                pos.x, pos.y, pos.z, rot.x, rot.y, rot.z, fov);

            try
            {
                File.WriteAllText(filePath, line);
            }
            catch { }
        }

        void RunServer()
        {
            try
            {
                listener = new TcpListener(IPAddress.Loopback, 9123);
                listener.Start();
                Logger.LogInfo("CamExport TCP server on port 9123");

                while (true)
                {
                    var client = listener.AcceptTcpClient();
                    var stream = client.GetStream();

                    var cam = Camera.main;
                    string response = "no camera";
                    if (cam != null)
                    {
                        var pos = cam.transform.position;
                        var rot = cam.transform.eulerAngles;
                        response = string.Format("{0:F4} {1:F4} {2:F4} {3:F4} {4:F4} {5:F4} {6:F4}",
                            pos.x, pos.y, pos.z, rot.x, rot.y, rot.z, cam.fieldOfView);
                    }

                    var bytes = Encoding.UTF8.GetBytes(response + "\n");
                    stream.Write(bytes, 0, bytes.Length);
                    client.Close();
                }
            }
            catch (System.Exception e)
            {
                Logger.LogError("CamExport server error: " + e);
            }
        }

        void OnDestroy()
        {
            listener?.Stop();
        }
    }
}
