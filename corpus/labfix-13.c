/* ЛБ4, задача 2: матрица из случайных чисел.
 * Максимум и минимум, транспонирование в новую матрицу. */
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define N_MAX 100

void gen_mas(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;

    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            a[i][j] = rand() % 21 - 10;
        }
    }
}

void show_mas(int a[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;

    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            printf("%5d", a[i][j]);
        }
        printf("\n");
    }
}

void transpose(int src[][N_MAX], int dst[][N_MAX], int n, int m) {
    int i = 0;
    int j = 0;

    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            dst[j][i] = src[i][j];
        }
    }
}

int main(void) {
    int a[N_MAX][N_MAX];
    int b[N_MAX][N_MAX];
    int n = 0;
    int m = 0;
    int i = 0;
    int j = 0;
    int max = 0;

    while (1) {
        printf("n m (1..%d): ", N_MAX);
        if (scanf("%d %d", &n, &m) == 2 && n >= 1 && n <= N_MAX && m >= 1 && m <= N_MAX) {
            break;
        }
        printf("плохой размер, повтори\n");
        while (getchar() != '\n') {
        }
    }
    srand(time(NULL));
    gen_mas(a, n, m);
    printf("исходная матрица:\n");
    show_mas(a, n, m);
    max = a[0][0];
    for (i = 0; i < n; i++) {
        for (j = 0; j < m; j++) {
            if (a[i][j] > max) {
                max = a[i][j];
            }
        }
    }
    printf("максимум = %d\n", max);
    transpose(a, b, n, m);
    printf("транспонированная:\n");
    show_mas(b, m, n);
    return 0;
}
