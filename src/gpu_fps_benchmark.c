#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <time.h>
#include <unistd.h>
#include <EGL/egl.h>
#include <GLES2/gl2.h>

static double get_time_sec(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec * 1e-9;
}

static void read_gpu_utilization(char *buf, size_t sz) {
    FILE *f = fopen("/proc/pvr/status", "r");
    if (!f) { snprintf(buf, sz, "N/A"); return; }
    char line[128];
    buf[0] = '\0';
    while (fgets(line, sizeof(line), f)) {
        if (strstr(line, "GPU Utilisation:")) {
            char *p = strchr(line, ':');
            if (p) {
                while (*(++p) == ' ');
                line[strcspn(line, "\r\n")] = 0;
                snprintf(buf, sz, "%s", p);
                break;
            }
        }
    }
    fclose(f);
    if (!buf[0]) snprintf(buf, sz, "0%%");
}

static int read_gpu_frequency_mhz(void) {
    FILE *f = fopen("/sys/kernel/ged/hal/current_freqency", "r");
    if (!f) return 0;
    int opp = 0, khz = 0;
    fscanf(f, "%d %d", &opp, &khz);
    fclose(f);
    return khz / 1000;
}

static const char *vert_shader_src =
    "attribute vec4 aPos;\n"
    "attribute vec3 aNormal;\n"
    "varying vec3 vNormal;\n"
    "varying vec3 vPos;\n"
    "uniform mat4 uMVP;\n"
    "uniform mat4 uModel;\n"
    "void main() {\n"
    "    gl_Position = uMVP * aPos;\n"
    "    vNormal = mat3(uModel[0].xyz, uModel[1].xyz, uModel[2].xyz) * aNormal;\n"
    "    vPos = (uModel * aPos).xyz;\n"
    "}\n";

static const char *frag_shader_src =
    "precision mediump float;\n"
    "varying vec3 vNormal;\n"
    "varying vec3 vPos;\n"
    "uniform vec3 uLightPos;\n"
    "uniform vec3 uColor;\n"
    "void main() {\n"
    "    vec3 N = normalize(vNormal);\n"
    "    vec3 L = normalize(uLightPos - vPos);\n"
    "    float diff = max(dot(N, L), 0.15);\n"
    "    // Specular highlight\n"
    "    vec3 V = normalize(-vPos);\n"
    "    vec3 R = reflect(-L, N);\n"
    "    float spec = pow(max(dot(V, R), 0.0), 16.0);\n"
    "    vec3 color = uColor * diff + vec3(0.8) * spec;\n"
    "    gl_FragColor = vec4(color, 1.0);\n"
    "}\n";

static GLuint compile_s(GLenum type, const char *src) {
    GLuint s = glCreateShader(type);
    glShaderSource(s, 1, &src, NULL);
    glCompileShader(s);
    return s;
}

int main(int argc, char **argv) {
    int width = 720;
    int height = 1600;

    printf("======================================================================\n");
    printf("   POWERVR ROGUE GE8320 HARDWARE ACCELERATION BENCHMARK & FPS PROOF    \n");
    printf("======================================================================\n\n");

    EGLDisplay dpy = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    eglInitialize(dpy, NULL, NULL);

    EGLint cfg_attribs[] = {
        EGL_SURFACE_TYPE, EGL_PBUFFER_BIT,
        EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT,
        EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
        EGL_DEPTH_SIZE, 24,
        EGL_NONE
    };
    EGLConfig cfg;
    EGLint ncfg;
    eglChooseConfig(dpy, cfg_attribs, &cfg, 1, &ncfg);

    EGLint pb_attribs[] = { EGL_WIDTH, width, EGL_HEIGHT, height, EGL_NONE };
    EGLSurface surf = eglCreatePbufferSurface(dpy, cfg, pb_attribs);

    EGLint ctx_attribs[] = { EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE };
    EGLContext ctx = eglCreateContext(dpy, cfg, EGL_NO_CONTEXT, ctx_attribs);
    eglMakeCurrent(dpy, surf, surf, ctx);

    const char *vendor   = (const char *)glGetString(GL_VENDOR);
    const char *renderer = (const char *)glGetString(GL_RENDERER);
    const char *version  = (const char *)glGetString(GL_VERSION);

    printf("[*] Hardware Identification:\n");
    printf("    - GPU Vendor:      %s\n", vendor);
    printf("    - GPU Chipset:     %s\n", renderer);
    printf("    - DDK Driver:      %s\n", version);
    printf("    - Display Buffer:  %dx%d (1,152,000 pixels/frame, 32-bit RGBA)\n\n", width, height);

    char util_buf[64];
    read_gpu_utilization(util_buf, sizeof(util_buf));
    int freq = read_gpu_frequency_mhz();
    printf("[*] Baseline Hardware State (Idle):\n");
    printf("    - GPU Utilisation: %s\n", util_buf);
    printf("    - GPU Frequency:   %d MHz\n\n", freq);

    GLuint vs = compile_s(GL_VERTEX_SHADER, vert_shader_src);
    GLuint fs = compile_s(GL_FRAGMENT_SHADER, frag_shader_src);
    GLuint prog = glCreateProgram();
    glAttachShader(prog, vs);
    glAttachShader(prog, fs);
    glLinkProgram(prog);
    glUseProgram(prog);

    GLint aPos    = glGetAttribLocation(prog, "aPos");
    GLint aNorm   = glGetAttribLocation(prog, "aNormal");
    GLint uMVP    = glGetUniformLocation(prog, "uMVP");
    GLint uModel  = glGetUniformLocation(prog, "uModel");
    GLint uLight  = glGetUniformLocation(prog, "uLightPos");
    GLint uColor  = glGetUniformLocation(prog, "uColor");

    // 3D Cube vertices (36 vertices)
    GLfloat cube_verts[] = {
        // Front
        -0.5f, -0.5f,  0.5f,  0.5f, -0.5f,  0.5f,  0.5f,  0.5f,  0.5f,
        -0.5f, -0.5f,  0.5f,  0.5f,  0.5f,  0.5f, -0.5f,  0.5f,  0.5f,
        // Back
        -0.5f, -0.5f, -0.5f, -0.5f,  0.5f, -0.5f,  0.5f,  0.5f, -0.5f,
        -0.5f, -0.5f, -0.5f,  0.5f,  0.5f, -0.5f,  0.5f, -0.5f, -0.5f,
        // Top
        -0.5f,  0.5f, -0.5f, -0.5f,  0.5f,  0.5f,  0.5f,  0.5f,  0.5f,
        -0.5f,  0.5f, -0.5f,  0.5f,  0.5f,  0.5f,  0.5f,  0.5f, -0.5f,
        // Bottom
        -0.5f, -0.5f, -0.5f,  0.5f, -0.5f, -0.5f,  0.5f, -0.5f,  0.5f,
        -0.5f, -0.5f, -0.5f,  0.5f, -0.5f,  0.5f, -0.5f, -0.5f,  0.5f,
        // Right
         0.5f, -0.5f, -0.5f,  0.5f,  0.5f, -0.5f,  0.5f,  0.5f,  0.5f,
         0.5f, -0.5f, -0.5f,  0.5f,  0.5f,  0.5f,  0.5f, -0.5f,  0.5f,
        // Left
        -0.5f, -0.5f, -0.5f, -0.5f, -0.5f,  0.5f, -0.5f,  0.5f,  0.5f,
        -0.5f, -0.5f, -0.5f, -0.5f,  0.5f,  0.5f, -0.5f,  0.5f, -0.5f
    };

    GLfloat cube_norms[] = {
        0,0,1,  0,0,1,  0,0,1,  0,0,1,  0,0,1,  0,0,1,
        0,0,-1, 0,0,-1, 0,0,-1, 0,0,-1, 0,0,-1, 0,0,-1,
        0,1,0,  0,1,0,  0,1,0,  0,1,0,  0,1,0,  0,1,0,
        0,-1,0, 0,-1,0, 0,-1,0, 0,-1,0, 0,-1,0, 0,-1,0,
        1,0,0,  1,0,0,  1,0,0,  1,0,0,  1,0,0,  1,0,0,
        -1,0,0, -1,0,0, -1,0,0, -1,0,0, -1,0,0, -1,0,0
    };

    glVertexAttribPointer(aPos, 3, GL_FLOAT, GL_FALSE, 0, cube_verts);
    glEnableVertexAttribArray(aPos);
    glVertexAttribPointer(aNorm, 3, GL_FLOAT, GL_FALSE, 0, cube_norms);
    glEnableVertexAttribArray(aNorm);

    glViewport(0, 0, width, height);
    glEnable(GL_DEPTH_TEST);
    glUniform3f(uLight, 2.0f, 4.0f, 3.0f);
    glUniform3f(uColor, 0.2f, 0.7f, 1.0f);

    int total_frames = 500;
    printf("[*] Executing Heavy 3D Shading Benchmark (%d frames with 3D Depth + Specular Lighting)...\n", total_frames);

    double t0 = get_time_sec();
    for (int i = 0; i < total_frames; i++) {
        float rad = i * 0.04f;
        float c = cosf(rad);
        float s = sinf(rad);
        float aspect = (float)width / (float)height;

        GLfloat model[16] = {
             c,  0.0f,   s,  0.0f,
            0.0f, 1.0f, 0.0f, 0.0f,
            -s,  0.0f,   c,  0.0f,
            0.0f, 0.0f, -2.5f, 1.0f
        };

        GLfloat mvp[16] = {
            c * aspect, 0.0f, s * 0.5f, 0.0f,
            0.0f,       1.0f, 0.0f,     0.0f,
           -s * aspect, 0.0f, c * 0.5f, 0.0f,
            0.0f,       0.0f, 0.5f,     1.0f
        };

        glUniformMatrix4fv(uModel, 1, GL_FALSE, model);
        glUniformMatrix4fv(uMVP, 1, GL_FALSE, mvp);

        glClearColor(0.08f, 0.10f, 0.16f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

        glDrawArrays(GL_TRIANGLES, 0, 36);
        glFinish();
    }
    double t1 = get_time_sec();
    double elapsed = t1 - t0;
    double fps = total_frames / elapsed;

    read_gpu_utilization(util_buf, sizeof(util_buf));
    freq = read_gpu_frequency_mhz();

    printf("\n======================================================================\n");
    printf("                           BENCHMARK RESULTS                          \n");
    printf("======================================================================\n");
    printf("  Total Rendered Frames:     %d frames\n", total_frames);
    printf("  Total Execution Time:      %.3f seconds\n", elapsed);
    printf("  Average Frame Time:        %.2f ms\n", (elapsed / total_frames) * 1000.0);
    printf("  ==============================================================\n");
    printf("  >>> RAW HARDWARE GPU PERFORMANCE:  %.1f FPS <<<\n", fps);
    printf("  ==============================================================\n");
    printf("  GPU Hardware Utilisation:  %s (Active load during test)\n", util_buf);
    printf("  GPU Frequency:             %d MHz\n", freq);
    printf("======================================================================\n");

    eglDestroyContext(dpy, ctx);
    eglDestroySurface(dpy, surf);
    eglTerminate(dpy);
    return 0;
}
